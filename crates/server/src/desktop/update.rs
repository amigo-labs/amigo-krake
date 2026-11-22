//! The desktop app's update controller (`docs/specs/desktop-app.md`, P3):
//! when background checks run (AC-13, AC-27), what the update choice does
//! (AC-15, AC-25 to AC-31, AC-34, AC-35) and when a copy may update itself
//! (AC-16). The download, signature check and install are the backend's
//! (tauri-plugin-updater in the app, a fake in tests).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::state::{DesktopState, UpdateMode};
use crate::update_api::{DesktopModeState, DesktopUpdateInfo, DesktopUpdater};

/// How long the first background check waits after start (AC-13).
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(10);
/// How often the background loop looks whether a check is due.
pub const TICK: Duration = Duration::from_secs(60);

/// Download, verify and install; implemented on top of tauri-plugin-updater.
#[async_trait::async_trait]
pub trait UpdateBackend: Send + Sync + 'static {
    /// Fetch `latest.json`; `None` when this copy is current.
    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String>;
    /// Download `info`'s version, verify its signature against the
    /// configured key and install it (macOS bundle, AppImage) or hold it for
    /// the app's exit (Windows). A failed verification installs nothing.
    async fn download_and_install(&self, info: &DesktopUpdateInfo) -> Result<(), String>;
    /// The app is ending: run what was held for this moment.
    fn on_exit(&self) {}
}

/// Whether a copy may replace itself (AC-16): never in debug builds, and on
/// Linux only as an AppImage (`APPIMAGE` set); `.deb` / `.rpm` installs
/// belong to the package manager.
pub fn can_self_update(debug_build: bool, os: &str, appimage: Option<&str>) -> bool {
    if debug_build {
        return false;
    }
    match os {
        "linux" => appimage.is_some_and(|p| !p.is_empty()),
        "windows" | "macos" => true,
        _ => false,
    }
}

/// Whether a background check is due at `now` (AC-13): never under "never
/// check"; at once when none is recorded or the clock went back; otherwise
/// once `interval_hours` have passed.
pub fn is_due(mode: UpdateMode, last_check: u64, now: u64, interval_hours: u64) -> bool {
    if mode == UpdateMode::Off {
        return false;
    }
    if last_check == 0 || now < last_check {
        return true;
    }
    now - last_check >= interval_hours.max(1) * 3600
}

struct Inner {
    /// What `desktop.json` holds (only stored values).
    stored: DesktopState,
    /// The choice confirmed or selected in this run, even if storing failed.
    session_mode: Option<UpdateMode>,
    /// The first-start question was closed without a choice in this run.
    question_closed: bool,
    /// Downloaded and verified in this run; takes effect at the next start.
    pending_restart: Option<String>,
    /// Found under "notify" and not yet announced.
    announcement: Option<String>,
    /// The most recent check's result and its generation.
    last_check: Option<(u64, Result<Option<DesktopUpdateInfo>, String>)>,
}

/// The desktop updater: implements [`DesktopUpdater`] for the server and
/// runs the background checks.
pub struct UpdateController<B: UpdateBackend> {
    backend: B,
    state_path: PathBuf,
    can_self_update: bool,
    interval_hours: u64,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    inner: Mutex<Inner>,
    /// One check (one request to `latest.json`) at a time; waiters share it.
    check_gate: tokio::sync::Mutex<()>,
    /// Counts finished checks, so a waiter can tell one finished meanwhile.
    check_generation: std::sync::atomic::AtomicU64,
    /// One download at a time; a second apply of the same version shares it.
    apply_gate: tokio::sync::Mutex<()>,
    /// Wakes the background loop when the choice or the question changes.
    wake: tokio::sync::Notify,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl<B: UpdateBackend> UpdateController<B> {
    pub fn new(
        backend: B,
        state_path: PathBuf,
        can_self_update: bool,
        interval_hours: u64,
    ) -> Self {
        let stored = DesktopState::load(&state_path);
        Self {
            backend,
            state_path,
            can_self_update,
            interval_hours,
            clock: Box::new(unix_now),
            inner: Mutex::new(Inner {
                stored,
                session_mode: None,
                question_closed: false,
                pending_restart: None,
                announcement: None,
                last_check: None,
            }),
            check_gate: tokio::sync::Mutex::new(()),
            check_generation: std::sync::atomic::AtomicU64::new(0),
            apply_gate: tokio::sync::Mutex::new(()),
            wake: tokio::sync::Notify::new(),
        }
    }

    /// Replace the clock (Unix seconds), for tests.
    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn effective_mode(inner: &Inner) -> UpdateMode {
        inner
            .session_mode
            .or(inner.stored.update_mode)
            .unwrap_or(UpdateMode::Auto)
    }

    fn question_pending(&self, inner: &Inner) -> bool {
        self.can_self_update
            && inner.stored.update_mode.is_none()
            && inner.session_mode.is_none()
            && !inner.question_closed
    }

    fn snapshot(&self, inner: &Inner, saved: bool) -> DesktopModeState {
        DesktopModeState {
            update_mode: Self::effective_mode(inner),
            update_mode_chosen: inner.stored.update_mode.is_some(),
            question_pending: self.question_pending(inner),
            can_self_update: self.can_self_update,
            pending_restart: inner.pending_restart.clone(),
            announcement: inner.announcement.clone(),
            saved,
        }
    }

    /// Store `change` applied to the stored state; on a failed write the
    /// stored state stays as it was. Returns whether the write worked.
    fn store(&self, inner: &mut Inner, change: impl FnOnce(&mut DesktopState)) -> bool {
        let mut next = inner.stored.clone();
        change(&mut next);
        match next.save(&self.state_path) {
            Ok(()) => {
                inner.stored = next;
                true
            }
            Err(e) => {
                tracing::warn!("Could not write {}: {e}", self.state_path.display());
                false
            }
        }
    }

    /// One check shared by everyone who asked while it ran (edge case:
    /// banner, About and background at once).
    async fn shared_check(&self) -> Result<Option<DesktopUpdateInfo>, String> {
        use std::sync::atomic::Ordering;
        let asked_at = self.check_generation.load(Ordering::SeqCst);
        let _gate = self.check_gate.lock().await;
        // A check finished while this caller waited: share its answer.
        if let Some((generation, result)) = &self.lock().last_check
            && *generation > asked_at
        {
            return result.clone();
        }
        let result = self.backend.check().await;
        let generation = self.check_generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.lock().last_check = Some((generation, result.clone()));
        result
    }

    /// Download, verify and install `info`, once per version.
    async fn install(&self, info: &DesktopUpdateInfo) -> Result<String, String> {
        let _gate = self.apply_gate.lock().await;
        if self.lock().pending_restart.as_deref() == Some(info.version.as_str()) {
            return Ok(info.version.clone());
        }
        self.backend.download_and_install(info).await?;
        self.lock().pending_restart = Some(info.version.clone());
        tracing::info!(
            "Krake {} is ready and takes effect at the next start",
            info.version
        );
        Ok(info.version.clone())
    }

    /// One pass of the background loop: check if due and act on the
    /// choice (AC-13, AC-15, AC-27, AC-28). Returns whether a check ran.
    pub async fn tick(&self) -> bool {
        if !self.can_self_update {
            return false;
        }
        let mode = {
            let mut inner = self.lock();
            let mode = Self::effective_mode(&inner);
            let now = (self.clock)();
            if self.question_pending(&inner)
                || !is_due(
                    mode,
                    inner.stored.last_update_check,
                    now,
                    self.interval_hours,
                )
            {
                return false;
            }
            self.store(&mut inner, |s| s.last_update_check = now);
            mode
        };
        match self.shared_check().await {
            Err(e) => tracing::warn!("Update check failed: {e}"),
            Ok(None) => {}
            Ok(Some(info)) => match mode {
                UpdateMode::Auto => {
                    if let Err(e) = self.install(&info).await {
                        tracing::warn!("Update to {} not installed: {e}", info.version);
                    }
                }
                UpdateMode::Notify => {
                    let mut inner = self.lock();
                    if inner.stored.announced_version.as_deref() != Some(info.version.as_str())
                        && inner.pending_restart.as_deref() != Some(info.version.as_str())
                    {
                        inner.announcement = Some(info.version.clone());
                    }
                }
                UpdateMode::Off => {}
            },
        }
        true
    }

    /// Run the background checks until the process ends: the first after
    /// [`FIRST_CHECK_DELAY`], then every [`TICK`] or when woken.
    pub async fn run_background(self: Arc<Self>) {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            self.tick().await;
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = self.wake.notified() => {}
            }
        }
    }

    /// The window was hidden (closed to the tray): an open first-start
    /// question counts as closed without a choice (AC-26).
    pub fn window_hidden(&self) {
        let pending = {
            let inner = self.lock();
            self.question_pending(&inner)
        };
        if pending {
            self.dismiss_question();
        }
    }

    /// The app is ending (Windows: run the held installer).
    pub fn on_exit(&self) {
        self.backend.on_exit();
    }
}

#[async_trait::async_trait]
impl<B: UpdateBackend> DesktopUpdater for UpdateController<B> {
    fn can_self_update(&self) -> bool {
        self.can_self_update
    }

    fn pending_restart(&self) -> Option<String> {
        self.lock().pending_restart.clone()
    }

    async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String> {
        self.shared_check().await
    }

    async fn apply(&self) -> Result<String, String> {
        if !self.can_self_update {
            return Err("this copy cannot update itself".into());
        }
        match self.shared_check().await? {
            Some(info) => self.install(&info).await,
            None => match self.pending_restart() {
                Some(version) => Ok(version),
                None => Err("Krake is up to date".into()),
            },
        }
    }

    fn mode_state(&self) -> DesktopModeState {
        let inner = self.lock();
        self.snapshot(&inner, true)
    }

    fn set_mode(&self, mode: UpdateMode) -> DesktopModeState {
        let state = {
            let mut inner = self.lock();
            inner.session_mode = Some(mode);
            inner.question_closed = true;
            let saved = self.store(&mut inner, |s| s.update_mode = Some(mode));
            if mode != UpdateMode::Notify {
                inner.announcement = None;
            }
            self.snapshot(&inner, saved)
        };
        self.wake.notify_one();
        state
    }

    fn dismiss_question(&self) {
        self.lock().question_closed = true;
        self.wake.notify_one();
    }

    fn acknowledge(&self, version: &str) -> Result<(), String> {
        let mut inner = self.lock();
        if inner.announcement.as_deref() != Some(version) {
            return Err(format!("{version} is not the pending announcement"));
        }
        inner.announcement = None;
        let version = version.to_string();
        if !self.store(&mut inner, |s| s.announced_version = Some(version)) {
            tracing::warn!("The announcement could not be stored; it may show again");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    #[derive(Default)]
    struct Fake {
        latest: Mutex<Option<String>>,
        checks: AtomicUsize,
        installs: AtomicUsize,
        fail_install: Mutex<Option<String>>,
        check_delay_ms: u64,
    }

    #[async_trait::async_trait]
    impl UpdateBackend for Arc<Fake> {
        async fn check(&self) -> Result<Option<DesktopUpdateInfo>, String> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            if self.check_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(self.check_delay_ms)).await;
            }
            Ok(self
                .latest
                .lock()
                .unwrap()
                .clone()
                .map(|version| DesktopUpdateInfo {
                    version,
                    notes: None,
                }))
        }
        async fn download_and_install(&self, _info: &DesktopUpdateInfo) -> Result<(), String> {
            if let Some(e) = self.fail_install.lock().unwrap().clone() {
                return Err(e);
            }
            self.installs.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(())
        }
    }

    struct Setup {
        fake: Arc<Fake>,
        ctl: Arc<UpdateController<Arc<Fake>>>,
        now: Arc<AtomicU64>,
        dir: tempfile::TempDir,
    }

    fn setup_with(state: Option<DesktopState>, can_update: bool, fake: Fake) -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desktop.json");
        if let Some(state) = state {
            state.save(&path).unwrap();
        }
        let fake = Arc::new(fake);
        let now = Arc::new(AtomicU64::new(1_000_000));
        let clock = now.clone();
        let ctl = Arc::new(
            UpdateController::new(fake.clone(), path, can_update, 24)
                .with_clock(move || clock.load(Ordering::SeqCst)),
        );
        Setup {
            fake,
            ctl,
            now,
            dir,
        }
    }

    fn chosen(mode: UpdateMode) -> Option<DesktopState> {
        Some(DesktopState {
            update_mode: Some(mode),
            ..Default::default()
        })
    }

    fn stored(s: &Setup) -> DesktopState {
        DesktopState::load(&s.dir.path().join("desktop.json"))
    }

    #[test]
    fn ac16_self_update_only_in_release_and_never_from_deb_or_rpm() {
        assert!(!can_self_update(true, "windows", None));
        assert!(!can_self_update(true, "linux", Some("/x/Krake.AppImage")));
        assert!(can_self_update(false, "windows", None));
        assert!(can_self_update(false, "macos", None));
        assert!(can_self_update(false, "linux", Some("/x/Krake.AppImage")));
        assert!(!can_self_update(false, "linux", None));
        assert!(!can_self_update(false, "linux", Some("")));
    }

    #[test]
    fn ac13_is_due_after_the_interval_or_when_the_clock_went_back() {
        let h = 3600;
        assert!(is_due(UpdateMode::Auto, 0, 100, 24), "nothing recorded");
        assert!(!is_due(UpdateMode::Auto, 1000, 1000 + 23 * h, 24));
        assert!(is_due(UpdateMode::Auto, 1000, 1000 + 24 * h, 24));
        assert!(is_due(UpdateMode::Notify, 1000, 1000 + 25 * h, 24));
        assert!(is_due(UpdateMode::Auto, 1000, 500, 24), "clock went back");
        assert!(!is_due(UpdateMode::Off, 0, 100, 24), "never check");
    }

    #[tokio::test]
    async fn ac25_ac34_a_fresh_copy_asks_and_answers_without_the_network() {
        let s = setup_with(None, true, Fake::default());
        let state = s.ctl.mode_state();
        assert!(state.question_pending);
        assert!(!state.update_mode_chosen);
        assert_eq!(state.update_mode, UpdateMode::Auto);
        assert!(state.can_self_update);
        assert_eq!(
            s.fake.checks.load(Ordering::SeqCst),
            0,
            "no network for the state"
        );

        // A copy that cannot update itself never asks.
        let s = setup_with(None, false, Fake::default());
        assert!(!s.ctl.mode_state().question_pending);
    }

    #[tokio::test]
    async fn ac26_confirm_stores_and_closing_stores_nothing() {
        let s = setup_with(None, true, Fake::default());
        let state = s.ctl.set_mode(UpdateMode::Notify);
        assert!(state.saved && state.update_mode_chosen && !state.question_pending);
        assert_eq!(stored(&s).update_mode, Some(UpdateMode::Notify));

        let s = setup_with(None, true, Fake::default());
        s.ctl.dismiss_question();
        let state = s.ctl.mode_state();
        assert!(!state.question_pending);
        assert_eq!(
            state.update_mode,
            UpdateMode::Auto,
            "automatic for this run"
        );
        assert_eq!(stored(&s).update_mode, None, "asked again next start");

        // Hiding the window to the tray counts as closing.
        let s = setup_with(None, true, Fake::default());
        s.ctl.window_hidden();
        assert!(!s.ctl.mode_state().question_pending);
        assert_eq!(stored(&s).update_mode, None);
    }

    #[tokio::test]
    async fn ac26_ac31_a_failed_write_still_applies_for_this_run() {
        let s = setup_with(None, true, Fake::default());
        // Make the state path unwritable: a directory in its place.
        std::fs::create_dir_all(s.dir.path().join("desktop.json")).unwrap();
        let state = s.ctl.set_mode(UpdateMode::Off);
        assert!(!state.saved);
        assert_eq!(state.update_mode, UpdateMode::Off);
        assert!(!state.update_mode_chosen);
        assert!(!state.question_pending);
    }

    #[tokio::test]
    async fn ac27_no_background_check_while_the_question_is_open() {
        let s = setup_with(None, true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        assert!(!s.ctl.tick().await);
        assert_eq!(s.fake.checks.load(Ordering::SeqCst), 0);
        s.ctl.dismiss_question();
        assert!(s.ctl.tick().await, "a due check starts once it is closed");
        assert_eq!(s.fake.checks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ac13_ac15_auto_checks_when_due_and_installs() {
        let s = setup_with(chosen(UpdateMode::Auto), true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        assert!(s.ctl.tick().await);
        assert_eq!(s.fake.installs.load(Ordering::SeqCst), 1);
        assert_eq!(s.ctl.pending_restart().as_deref(), Some("2.0.0"));
        assert_eq!(stored(&s).last_update_check, 1_000_000);

        // Not due again within the interval.
        s.now.fetch_add(3600, Ordering::SeqCst);
        assert!(!s.ctl.tick().await);
        // Due once the interval passed.
        s.now.fetch_add(24 * 3600, Ordering::SeqCst);
        assert!(s.ctl.tick().await);
        assert_eq!(
            s.fake.installs.load(Ordering::SeqCst),
            1,
            "same version not again"
        );
    }

    #[tokio::test]
    async fn ac13_ac16_no_background_check_under_never_or_without_self_update() {
        let s = setup_with(chosen(UpdateMode::Off), true, Fake::default());
        assert!(!s.ctl.tick().await);
        let s = setup_with(chosen(UpdateMode::Auto), false, Fake::default());
        assert!(!s.ctl.tick().await);
        assert_eq!(s.fake.checks.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn ac28_ac35_notify_announces_each_version_once() {
        let s = setup_with(chosen(UpdateMode::Notify), true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        s.ctl.tick().await;
        assert_eq!(
            s.fake.installs.load(Ordering::SeqCst),
            0,
            "notify downloads nothing"
        );
        assert_eq!(s.ctl.mode_state().announcement.as_deref(), Some("2.0.0"));

        // Only the pending version can be marked.
        assert!(s.ctl.acknowledge("1.9.0").is_err());
        s.ctl.acknowledge("2.0.0").unwrap();
        assert_eq!(s.ctl.mode_state().announcement, None);
        assert_eq!(stored(&s).announced_version.as_deref(), Some("2.0.0"));

        // The same version is not announced again; a newer one is.
        s.now.fetch_add(25 * 3600, Ordering::SeqCst);
        s.ctl.tick().await;
        assert_eq!(s.ctl.mode_state().announcement, None);
        *s.fake.latest.lock().unwrap() = Some("2.1.0".into());
        s.now.fetch_add(25 * 3600, Ordering::SeqCst);
        s.ctl.tick().await;
        assert_eq!(s.ctl.mode_state().announcement.as_deref(), Some("2.1.0"));
    }

    #[tokio::test]
    async fn ac28_an_unmarked_announcement_comes_back_after_a_restart() {
        let s = setup_with(chosen(UpdateMode::Notify), true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        s.ctl.tick().await;
        assert!(s.ctl.mode_state().announcement.is_some());
        // Quit with it open: nothing stored, so a new run announces it again.
        let path = s.dir.path().join("desktop.json");
        let again = UpdateController::new(s.fake.clone(), path, true, 24);
        again.tick().await;
        assert_eq!(again.mode_state().announcement.as_deref(), Some("2.0.0"));
    }

    #[tokio::test]
    async fn ac29_never_check_still_allows_a_manual_apply() {
        let s = setup_with(chosen(UpdateMode::Off), true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        assert_eq!(s.ctl.apply().await.unwrap(), "2.0.0");
        assert_eq!(s.ctl.mode_state().announcement, None);
        assert_eq!(s.ctl.pending_restart().as_deref(), Some("2.0.0"));
    }

    #[tokio::test]
    async fn ac14_a_failed_verification_installs_nothing() {
        let s = setup_with(chosen(UpdateMode::Auto), true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        *s.fake.fail_install.lock().unwrap() = Some("signature mismatch".into());
        assert_eq!(s.ctl.apply().await.unwrap_err(), "signature mismatch");
        assert_eq!(s.ctl.pending_restart(), None);
        s.ctl.tick().await;
        assert_eq!(s.ctl.pending_restart(), None);
    }

    #[tokio::test]
    async fn ac30_choices_take_effect_without_a_restart() {
        let s = setup_with(chosen(UpdateMode::Off), true, Fake::default());
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        assert!(!s.ctl.tick().await);
        s.ctl.set_mode(UpdateMode::Notify);
        assert!(s.ctl.tick().await, "a due check runs at once");
        assert_eq!(s.ctl.mode_state().announcement.as_deref(), Some("2.0.0"));
        s.ctl.set_mode(UpdateMode::Off);
        assert_eq!(s.ctl.mode_state().announcement, None);
    }

    #[tokio::test]
    async fn concurrent_checks_and_applies_share_one_request_and_one_download() {
        let fake = Fake {
            check_delay_ms: 50,
            ..Default::default()
        };
        let s = setup_with(chosen(UpdateMode::Notify), true, fake);
        *s.fake.latest.lock().unwrap() = Some("2.0.0".into());
        let (a, b, c) = tokio::join!(s.ctl.check(), s.ctl.apply(), s.ctl.apply());
        assert!(a.unwrap().is_some());
        assert_eq!(b.unwrap(), "2.0.0");
        assert_eq!(c.unwrap(), "2.0.0");
        assert_eq!(s.fake.checks.load(Ordering::SeqCst), 1, "one request");
        assert_eq!(s.fake.installs.load(Ordering::SeqCst), 1, "one download");
    }
}
