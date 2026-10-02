#![cfg(all(feature = "schema-validation", not(target_arch = "wasm32")))]
use hitslop_core::{manifest, shape::Silhouette, Code, PACKAGE_FORMAT, RUNTIME_ABI};

fn fixture() -> serde_json::Value {
    serde_json::json!({"author":{"name":"Author"},"slug":"fixture","title":"Title","description":"Description","categories":["utilities"],"presentation":{"width":320,"height":240},"packageFormat":1,"runtimeABI":1})
}
/// A built package's manifest: the authored one plus the level `slop build` stamps.
fn built(mut authored: serde_json::Value) -> String {
    authored["packageFormat"] = serde_json::json!(PACKAGE_FORMAT);
    authored["runtimeABI"] = serde_json::json!(RUNTIME_ABI);
    authored.to_string()
}

#[test]
fn a_package_names_its_platform_and_a_newer_one_asks_for_an_update() {
    let mut value = fixture();
    value.as_object_mut().unwrap().remove("packageFormat");
    assert_eq!(manifest::validate(&value.to_string()).unwrap_err().code, Code::InvalidRequest, "built packages carry a level");
    for level in [serde_json::json!(0), serde_json::json!("1"), serde_json::json!(1.5)] {
        value["packageFormat"] = level;
        assert_eq!(manifest::validate(&value.to_string()).unwrap_err().code, Code::InvalidRequest);
    }
    // A newer package is refused as needing an update before its other fields are judged,
    // whatever fields this build does not know.
    let mut newer = fixture();
    newer["packageFormat"] = serde_json::json!(PACKAGE_FORMAT + 1);
    newer["presentation"]["future"] = serde_json::json!(true);
    newer["lineage"] = serde_json::json!({"template": "future"});
    let error = manifest::validate(&newer.to_string()).unwrap_err();
    assert_eq!(error.code, Code::RequiresUpdate, "{}", error.message);
}

#[test]
fn native_manifest_returns_geometry_and_located_bounded_errors() {
    let mut value = fixture();
    assert!(matches!(manifest::validate(&value.to_string()).unwrap(), Silhouette::Radii { .. }));
    value["presentation"]["shape"] = serde_json::json!({"path":"M0 0L100 100","viewBox":[100,100]});
    assert!(matches!(manifest::validate(&value.to_string()).unwrap(), Silhouette::Path { .. }));
    value["presentation"]["shape"] = serde_json::json!({"path":"M0 0X"});
    assert_eq!(manifest::validate(&value.to_string()).unwrap_err().code, Code::InvalidShape);
    value = fixture();
    value["title"] = serde_json::json!("secret".repeat(50));
    let error = manifest::validate(&value.to_string()).unwrap_err();
    assert!(error.message.contains("/title"));
    assert!(!error.message.contains("secret"));
    value = fixture();
    value["presentation"] = serde_json::json!({"width":320,"height":240,"skin":"assets/skin.png"});
    assert!(matches!(manifest::validate(&value.to_string()).unwrap(), Silhouette::Radii { .. }));
    for skin in ["assets/../skin.png", "assets/./skin.png", "assets//skin.png", "assets/skin.png\n"] {
        value["presentation"]["skin"] = serde_json::json!(skin);
        assert!(manifest::validate(&value.to_string()).is_err(), "{skin:?}");
    }
}

#[test]
fn native_manifest_bounds_json_and_preserves_unicode_semantics() {
    for count in [40, 41, 80, 81] {
        let mut value = fixture();
        value["title"] = serde_json::json!("😀".repeat(count));
        assert_eq!(manifest::validate(&value.to_string()).is_ok(), count <= 80);
    }
    let mut boundary = fixture().to_string();
    boundary.push_str(&" ".repeat(65536 - boundary.len()));
    assert!(manifest::validate(&boundary).is_ok());
    boundary.push(' ');
    assert_eq!(manifest::validate(&boundary).unwrap_err().code, Code::TooLarge);
    assert_eq!(manifest::validate("{").unwrap_err().code, Code::InvalidRequest);
}

/// The same corpus TypeBox is checked against (`packages/schema/tests/manifest.test.ts`):
/// complete acceptance is the schema plus the core's window-shape parser.
#[test]
fn native_manifest_agrees_with_the_shared_corpus() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packages/schema/tests/fixtures/manifest-cases.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(cases.len() > 80);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let accepted = manifest::validate(&built(case["manifest"].clone())).is_ok();
        assert_eq!(accepted, case["accept"].as_bool().unwrap(), "{name}");
    }
}

#[test]
fn native_manifest_accepts_every_active_example() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/slops");
    let mut checked = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let file = entry.unwrap().path().join("manifest.json");
        if let Ok(text) = std::fs::read_to_string(&file) {
            assert!(manifest::validate(&built(serde_json::from_str(&text).unwrap())).is_ok(), "{}", file.display());
            checked += 1;
        }
    }
    assert!(checked >= 10, "expected the active example manifests, found {checked}");
}

#[test]
fn package_syntax_and_runtime_requirements_are_independent() {
    for field in ["packageFormat", "runtimeABI"] {
        let mut newer = fixture();
        newer[field] = serde_json::json!(2);
        newer["unknown_future_field"] = serde_json::json!(true);
        assert_eq!(manifest::validate(&newer.to_string()).unwrap_err().code, Code::RequiresUpdate);
        let mut missing = fixture();
        missing.as_object_mut().unwrap().remove(field);
        assert_eq!(manifest::validate(&missing.to_string()).unwrap_err().code, Code::InvalidRequest);
    }
}
