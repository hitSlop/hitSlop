#![cfg(all(feature = "manifest-validation", not(target_arch = "wasm32")))]
use hitslop_core::{manifest, shape::Silhouette, Code};

fn fixture() -> serde_json::Value {
    serde_json::json!({"author":{"name":"Author"},"slug":"fixture","title":"Title","description":"Description","categories":["utilities"],"presentation":{"width":320,"height":240}})
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
