//! The link list rule against the shared fixtures (spec `link-list-import`,
//! AC-01 to AC-04). `web-ui/test/linkList.test.ts` reads the same files.

use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct Expected {
    links: Vec<String>,
    not_recognized: usize,
}

#[test]
fn link_list_fixtures() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/link-lists");
    let mut cases = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = std::fs::read(&path).unwrap();
        let expected: Expected = serde_json::from_str(
            &std::fs::read_to_string(dir.join(format!("{name}.expected.json"))).unwrap(),
        )
        .unwrap();
        let got = krake_core::linklist::parse(&bytes);
        if got.links != expected.links || got.not_recognized != expected.not_recognized {
            failures.push(format!(
                "{name}: got {:?} / {} not recognized, expected {:?} / {}",
                got.links, got.not_recognized, expected.links, expected.not_recognized
            ));
        }
        cases += 1;
    }
    assert!(cases >= 16, "expected the shared fixtures, found {cases}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
