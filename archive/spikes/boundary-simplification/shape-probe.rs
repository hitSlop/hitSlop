use hitslop_core::shape::{silhouette, Silhouette};
use serde_json::json;

#[test]
fn library_bounds_and_acceptance() {
    let mut results = vec![];
    for source in ["M,0 0", "M0 0,", "M0 0,Z", "M0 0Z 1 1", "M0 0A-20 -10 30 1 0 40 40Z", "M0 0A10 10 0 0 1 0 0Z", "M0 0L1e309 0", "M0 0A1e309 1 0 0 1 0 0"] {
        let accepted = silhouette(Some(&json!({"path": source}).to_string()), 100.0, 100.0).is_ok();
        results.push(json!({"source":source, "accepted":accepted}));
    }
    for radius in [50.0, 16384.0, 1e12, 1e22] {
        let source = format!("M0 0A{radius} {radius} 0 0 1 {} 0", radius * 2.0);
        let start = std::time::Instant::now();
        let raw = svgtypes::SimplifyingPathParser::from(source.as_str()).collect::<Result<Vec<_>,_>>().unwrap();
        let result = silhouette(Some(&json!({"path": source}).to_string()), 100.0, 100.0);
        let segments = match result { Ok(Silhouette::Path { segments, .. }) => Some(segments.len()), _ => None };
        results.push(json!({"radius":radius,"librarySegments":raw.len(),"adapterSegments":segments,"milliseconds":start.elapsed().as_secs_f64()*1000.0}));
    }
    std::fs::create_dir_all("../../.hitslop").unwrap();
    std::fs::write("../../.hitslop/boundary-shape.json", serde_json::to_vec_pretty(&results).unwrap()).unwrap();
}
