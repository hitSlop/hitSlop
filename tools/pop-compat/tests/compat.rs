use pop_testkit as support;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn entries(path: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(path).map(|dir| dir.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
    found.sort();
    found
}
fn json_file(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path).ok().map(|text| serde_json::from_str(&text).unwrap())
}
#[test]
fn no_recorded_open_limit_becomes_stricter_or_disappears() {
    let mut cases = 0;
    for entry in entries(&support::compat_root()).into_iter().filter(|path| path.join("release.json").exists()) {
        let release = json_file(&entry.join("release.json")).unwrap();
        pop_compat::check_recorded_limits(&release["limits"].to_string())
            .unwrap_or_else(|error| panic!("{}: {error}", entry.display()));
        cases += 1;
    }
    assert!(
        cases > 0 || std::env::var("POP_COMPAT_BOOTSTRAP").is_ok_and(|value| value == "1"),
        "Prepared compatibility corpus has no recorded limits"
    );
}

#[test]
fn every_stored_acceptance_vector_still_decodes_and_reencodes_equally() {
    check_stored_acceptance_vectors(false);
}

#[test]
fn large_acceptance_vectors_still_decode_and_reencode_equally() {
    check_stored_acceptance_vectors(true);
}

fn check_stored_acceptance_vectors(large: bool) {
    let mut cases = 0;
    for entry in entries(&support::compat_root()).into_iter().filter(|path| path.join("release.json").exists()) {
        let vectors: Vec<_> = entries(&entry.join("acceptance"))
            .into_iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
            .collect();
        assert!(!vectors.is_empty(), "{}: missing acceptance vectors", entry.display());
        for path in vectors {
            let stored = std::fs::read_to_string(&path).unwrap();
            if pop_compat::acceptance_vector_is_large(&stored)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                != large
            {
                continue;
            }
            pop_compat::check_acceptance_vector(&stored).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            cases += 1;
        }
    }
    assert!(
        cases > 0 || std::env::var("POP_COMPAT_BOOTSTRAP").is_ok_and(|value| value == "1"),
        "Prepared compatibility corpus has no acceptance vectors"
    );
}
