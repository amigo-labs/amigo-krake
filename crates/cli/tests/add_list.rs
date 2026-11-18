//! `krake add --list` as a process: standard input and exit status
//! (spec `link-list-import`, AC-19 to AC-21).

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Run `krake` in a fresh working directory (its `krake.db` lands there).
fn krake(dir: &std::path::Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_krake"))
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
