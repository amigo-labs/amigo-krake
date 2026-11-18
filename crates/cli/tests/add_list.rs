//! `krake add --list` as a process: standard input and exit status
//! (spec `link-list-import`, AC-19 to AC-21).

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Run `krake` in a fresh working directory (its `krake.db` lands there).
fn krake(dir: &std::path::Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    krake_in("C", dir, args, stdin)
}

/// Like [`krake`], with `LANG` set to `lang`.
fn krake_in(lang: &str, dir: &std::path::Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_krake"))
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .env("LANG", lang)
        .args(args)
        .current_dir(dir)
        .env("KRAKE_CONFIG_DIR", dir)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn krake");
    let mut input = child.stdin.take().unwrap();
    if let Some(data) = stdin {
        input.write_all(data).unwrap();
    }
    drop(input);
    child.wait_with_output().expect("wait for krake")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn list_from_stdin_is_queued() {
    // AC-19: `-` reads the list from standard input.
    let dir = tempfile::tempdir().unwrap();
    let out = krake(
        dir.path(),
        &["add", "--list", "-"],
        Some(b"https://host.example/a.rar\nhttps://host.example/b.rar\n"),
    );
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("queued 2"), "{}", text(&out));
}

#[test]
fn missing_file_fails_with_path() {
    // AC-20: unreadable file → path and reason, non-zero exit.
    let dir = tempfile::tempdir().unwrap();
    let out = krake(dir.path(), &["add", "--list", "nope.txt"], None);
    assert!(!out.status.success());
    assert!(text(&out).contains("nope.txt"), "{}", text(&out));
    let reason = std::fs::File::open(dir.path().join("nope.txt"))
        .unwrap_err()
        .to_string();
    assert!(
        text(&out).contains(&reason),
        "reason {reason:?} missing: {}",
        text(&out)
    );
}

/// The URLs `krake list --mode json` reports in `dir`.
fn listed_urls(dir: &std::path::Path) -> Vec<String> {
    // Without `NO_COLOR`, which would force plain output over `--mode json`.
    let out = Command::new(env!("CARGO_BIN_EXE_krake"))
        .args(["--mode", "json", "list"])
        .current_dir(dir)
        .env("KRAKE_CONFIG_DIR", dir)
        .env_remove("NO_COLOR")
        .output()
        .expect("run krake list");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("list output is not JSON ({e}): {}", text(&out)));
    json["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["url"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn oversized_list_with_links_queues_nothing() {
    // AC-20: a list over 1 MiB is refused as a whole, links and all.
    let dir = tempfile::tempdir().unwrap();
    let line = b"https://host.example/a.rar\n";
    let big: Vec<u8> = line
        .iter()
        .copied()
        .cycle()
        .take(1024 * 1024 + line.len())
        .collect();
    let out = krake(dir.path(), &["add", "--list", "-"], Some(&big));
    assert!(!out.status.success(), "{}", text(&out));
    assert!(listed_urls(dir.path()).is_empty(), "nothing may be queued");
}

#[test]
fn only_already_queued_links_exit_zero() {
    // AC-21: a list whose links are all queued already is not an error.
    let dir = tempfile::tempdir().unwrap();
    let list = b"https://host.example/a.rar\n";
    let first = krake(dir.path(), &["add", "--list", "-"], Some(list));
    assert!(first.status.success(), "{}", text(&first));
    assert_eq!(listed_urls(dir.path()), ["https://host.example/a.rar"]);
    let second = krake(dir.path(), &["add", "--list", "-"], Some(list));
    assert!(second.status.success(), "{}", text(&second));
    assert!(
        text(&second).contains("already queued 1"),
        "{}",
        text(&second)
    );
    assert!(text(&second).contains("queued 0"), "{}", text(&second));
}

#[test]
fn oversized_stdin_is_refused() {
    // AC-20: more than 1 MiB on standard input is refused.
    let dir = tempfile::tempdir().unwrap();
    let out = krake(
        dir.path(),
        &["add", "--list", "-"],
        Some(&vec![b'a'; 1024 * 1024 + 1]),
    );
    assert!(!out.status.success());
    assert!(text(&out).contains("1 MiB"), "{}", text(&out));
}

#[test]
fn list_without_links_fails() {
    // AC-21: no link → non-zero exit.
    let dir = tempfile::tempdir().unwrap();
    let out = krake(
        dir.path(),
        &["add", "--list", "-"],
        Some(b"# only a comment\nhello\n"),
    );
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("no links"), "{}", text(&out));
}

#[test]
fn every_link_rejected_fails() {
    // AC-21: every link rejected → non-zero exit.
    let dir = tempfile::tempdir().unwrap();
    let out = krake(
        dir.path(),
        &["add", "--list", "-"],
        Some(b"http://[bad/x\n"),
    );
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("rejected 1"), "{}", text(&out));
}

#[test]
fn messages_follow_the_system_language() {
    // AC-12: the CLI's link list messages exist in English and German, also
    // when krake runs outside the repository (no `locales/` next to it).
    let dir = tempfile::tempdir().unwrap();
    let list = b"https://host.example/a.rar\n";
    let de = krake_in(
        "de_DE.UTF-8",
        dir.path(),
        &["add", "--list", "-"],
        Some(list),
    );
    assert!(text(&de).contains("Link-Liste"), "{}", text(&de));
    let dir = tempfile::tempdir().unwrap();
    let en = krake_in(
        "en_US.UTF-8",
        dir.path(),
        &["add", "--list", "-"],
        Some(list),
    );
    assert!(text(&en).contains("Link list"), "{}", text(&en));
    let none = krake_in(
        "de_DE.UTF-8",
        dir.path(),
        &["add", "--list", "-"],
        Some(b"hallo\n"),
    );
    assert!(
        text(&none).contains("enthält keine Links"),
        "{}",
        text(&none)
    );
}
