//! `krake add --nzb` as a process: the NZB becomes one Usenet download.

use std::process::Command;

use krake_core::storage::Storage;

const NZB: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file poster="p" date="1" subject="&quot;a.part01.rar&quot; yEnc (1/1)">
    <groups><group>alt.binaries.test</group></groups>
    <segments><segment bytes="100" number="1">a1@test</segment></segments>
  </file>
  <file poster="p" date="1" subject="&quot;a.part02.rar&quot; yEnc (1/1)">
    <groups><group>alt.binaries.test</group></groups>
    <segments><segment bytes="200" number="1">a2@test</segment></segments>
  </file>
</nzb>"#;

#[tokio::test]
async fn an_nzb_file_is_added_as_one_usenet_download() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Some.Release.nzb");
    std::fs::write(&path, NZB).unwrap();
    // No Usenet server is configured, so nothing is fetched.
    let out = Command::new(env!("CARGO_BIN_EXE_krake"))
        .args(["add", "--nzb", path.to_str().unwrap()])
        .current_dir(dir.path())
        .env("KRAKE_CONFIG_DIR", dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("run krake");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");

    let storage = Storage::open(
        dir.path().join("krake.db"),
        dir.path().join("downloads"),
        dir.path().join("tmp"),
    )
    .unwrap();
    let rows = storage.list_downloads().await.unwrap();
    assert_eq!(rows.len(), 1, "one download for the NZB: {text}");
    let row = &rows[0];
    assert_eq!(row.protocol, "usenet");
    assert_eq!(row.filename.as_deref(), Some("Some.Release"));
    assert_eq!(row.filesize, Some(300));
    let metadata: serde_json::Value = serde_json::from_str(
        &storage
            .get_download_metadata(&row.id)
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["nzb_data"].as_str(), Some(NZB));
    assert!(text.contains(&row.id), "the id is printed: {text}");
}
