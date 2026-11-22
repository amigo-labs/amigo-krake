//! The Tauri side of the desktop app: the embedded server, the window, the
//! tray, single instance and deep links (`docs/specs/desktop-app.md`,
//! AC-1 to AC-12). Everything that needs no Tauri lives in
//! `krake_server::desktop`, where the workspace tests run it.

use std::sync::Arc;

use krake_core::coordinator::Coordinator;
use krake_server::desktop::links::{self, DeepLink};
use krake_server::desktop::paths::{self, DesktopPaths};
use krake_server::desktop::tray;
use krake_server::desktop_session::DESKTOP_PRINCIPAL;
use krake_server::embedded::{self, DesktopOptions, ServeOptions};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_notification::NotificationExt;

use crate::updater::TauriUpdater;

const MAIN_WINDOW: &str = "main";

/// What the running app keeps: the server's coordinator and updater.
struct Desktop {
    coordinator: Arc<Coordinator>,
    updater: Arc<TauriUpdater>,
}

pub fn run() {
    // Signal to krake-core that we are running inside Tauri, so
    // detect_distribution() returns Distribution::Tauri.
    // SAFETY: called at the very start of main, before any threads exist.
    unsafe {
        std::env::set_var("KRAKE_TAURI", "1");
    }
    tracing_subscriber::fmt().with_env_filter("info").init();
    init_locale();

    tauri::Builder::default()
        // First: a second launch hands its arguments to this one and exits
        // without starting a second server (AC-10).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            show_main(app);
            handle_args(app, argv.iter().map(String::as_str));
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("no app data directory: {e}"))?;
            let user_downloads = app
                .path()
                .download_dir()
                .unwrap_or_else(|_| data_dir.join("downloads"));
            let session_url =
                tauri::async_runtime::block_on(start_server(&handle, &data_dir, &user_downloads))?;

            WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::External(session_url))
                .title("Krake")
                .inner_size(1200.0, 800.0)
                .min_inner_size(400.0, 300.0)
                .build()?;

            build_tray(app.handle())?;

            // Links the app was started with, and macOS / runtime opens.
            let launch: Vec<String> = std::env::args().collect();
            handle_args(&handle, launch.iter().map(String::as_str));
            let on_open = handle.clone();
            app.deep_link().on_open_url(move |event| {
                show_main(&on_open);
                let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
                handle_args(&on_open, urls.iter().map(String::as_str));
            });
            Ok(())
        })
        // Closing the window hides it; only tray Quit ends the process
        // (AC-9). Closing it to the tray counts as closing the first-start
        // question without a choice (AC-26).
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == MAIN_WINDOW
            {
                api.prevent_close();
                let _ = window.hide();
                if let Some(desktop) = window.app_handle().try_state::<Desktop>() {
                    desktop.updater.window_hidden();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Error building the Krake desktop app")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(desktop) = app.try_state::<Desktop>()
            {
                // Windows: a held installer runs once the app has ended.
                desktop.updater.on_exit();
            }
        });
}

/// Bind, start the server and return the URL that signs the window in.
async fn start_server(
    app: &AppHandle,
    data_dir: &std::path::Path,
    user_downloads: &std::path::Path,
) -> Result<tauri::Url, Box<dyn std::error::Error>> {
    let paths = DesktopPaths::new(data_dir);
    paths.create_dirs()?;
    migrate_legacy_db(&paths);
    let config = paths.load_config(user_downloads)?;
    let (listener, _state) = paths::bind_listener(&paths.state_path).await?;

    let token = krake_server::login::new_session_id();
    let updater = Arc::new(TauriUpdater::new(app.clone(), paths.state_path.clone()));
    let handle = embedded::serve(ServeOptions {
        config,
        config_path: paths.config_path.clone(),
        db_path: paths.db_path.clone(),
        plugins_dir: paths.plugins_dir.clone(),
        cache_dir: paths.cache_dir.clone(),
        listener,
        start_clicknload: true,
        desktop: Some(DesktopOptions {
            session_token: token.clone(),
            updater: updater.clone(),
        }),
    })
    .await
    .map_err(|e| e.to_string())?;

    let url = format!(
        "http://{}/api/v1/desktop/session?token={token}",
        handle.local_addr
    );
    app.manage(Desktop {
        coordinator: handle.coordinator.clone(),
        updater: updater.clone(),
    });
    updater.start_background_checks();
    // The server runs for the life of the process; Quit ends both. Dropping
    // the handle does not stop it.
    drop(handle);
    Ok(tauri::Url::parse(&url)?)
}

/// Before the rename the database was `amigo.db`, and the app's data dir
/// was named after the identifier `com.amigo-labs.downloader`.
fn migrate_legacy_db(paths: &DesktopPaths) {
    let legacy = [
        Some(paths.app_data.join("amigo.db")),
        paths
            .app_data
            .parent()
            .map(|p| p.join("com.amigo-labs.downloader").join("amigo.db")),
    ];
    for legacy in legacy.into_iter().flatten() {
        if let Err(e) = krake_core::migrate::migrate_db(&legacy, &paths.db_path) {
            tracing::warn!(
                "Could not move {} to {}: {e}",
                legacy.display(),
                paths.db_path.display()
            );
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use krake_core::i18n::t;
    let item = |id: &str, key: &str| MenuItem::with_id(app, id, t(key), true, None::<&str>);
    let open = item("open", "desktop.tray.open")?;
    let pause = item("pause_all", "desktop.tray.pause_all")?;
    let resume = item("resume_all", "desktop.tray.resume_all")?;
    let quit = item("quit", "desktop.tray.quit")?;
    let menu = Menu::with_items(app, &[&open, &pause, &resume, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Krake")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "pause_all" => with_coordinator(app, |c| async move {
                let _ = tray::pause_all(&c).await;
            }),
            "resume_all" => with_coordinator(app, |c| async move {
                let _ = tray::resume_all(&c).await;
            }),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn with_coordinator<F, Fut>(app: &AppHandle, f: F)
where
    F: FnOnce(Arc<Coordinator>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    if let Some(desktop) = app.try_state::<Desktop>() {
        tauri::async_runtime::spawn(f(desktop.coordinator.clone()));
    }
}

/// Show, restore and focus the main window (AC-9, AC-10).
fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(desktop) = app.try_state::<Desktop>() {
            desktop.updater.window_shown();
        }
    }
}

/// Queue `krake://add` links and decline magnet links (AC-11, AC-12).
fn handle_args<'a>(app: &AppHandle, args: impl IntoIterator<Item = &'a str>) {
    for link in links::parse_all(args) {
        match link {
            DeepLink::Add(url) => {
                show_main(app);
                with_coordinator(app, move |c| async move {
                    if let Err(e) = c
                        .add_download_all(&url, None, None, 0, Some(DESKTOP_PRINCIPAL.to_string()))
                        .await
                    {
                        tracing::warn!("Could not add {url} from a link: {e}");
                    }
                });
            }
            DeepLink::Magnet => {
                let _ = app
                    .notification()
                    .builder()
                    .title("Krake")
                    .body(krake_core::i18n::t("desktop.magnet_unsupported"))
                    .show();
            }
            DeepLink::Malformed(reason) => tracing::warn!("Ignored link: {reason}"),
        }
    }
}

/// English or German, from the system language.
fn init_locale() {
    let lang = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    let lang = if lang.to_ascii_lowercase().starts_with("de") {
        "de"
    } else {
        "en"
    };
    let en = include_str!("../../locales/en.json");
    let de = include_str!("../../locales/de.json");
    krake_core::i18n::init_from_str(lang, if lang == "de" { de } else { en }, en);
}
