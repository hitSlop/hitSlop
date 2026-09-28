//! Diagnostic: physical footprint of one 1k/5k-row Loro document under variants.
use loro::{LoroDoc, ValueOrContainer, Container};
use std::sync::Arc;
fn footprint() -> f64 {
    let out = std::process::Command::new("footprint").arg(std::process::id().to_string()).output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().find(|l| l.contains("phys_footprint:")).unwrap_or("").to_string();
    let v: Vec<&str> = line.split_whitespace().collect();
    let (n, unit) = (v.get(1).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0), v.get(2).copied().unwrap_or(""));
    match unit { "KB" => n / 1024.0, "MB" => n, "GB" => n * 1024.0, _ => n / 1048576.0 }
}
fn edit(doc: &LoroDoc, n: usize) {
    let list = doc.get_map("data").get("rows").unwrap().into_container().unwrap().into_movable_list().unwrap();
    for i in 0..n {
        if let Some(ValueOrContainer::Container(Container::Map(m))) = list.get(i % list.len()) { m.insert("done", i % 2 == 0).unwrap(); }
        doc.commit();
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = std::fs::read(&args[1]).unwrap();
    let variant = args[2].as_str();
    let base = footprint();
    let mut doc = LoroDoc::new();
    doc.import(&seed).unwrap();
    let after_open = footprint();
    if variant.contains("sub") { doc.subscribe_root(Arc::new(|_e| {})).detach(); }
    if variant.contains("deep") { let _ = doc.get_map("data").get_deep_value(); }
    if variant.contains("fork") { for _ in 0..200 { edit(&doc, 1); let f = doc.fork(); doc = f; } } else { edit(&doc, 200); }
    println!("{variant}: base {base:.1} open +{:.1} final +{:.1} MiB", after_open - base, footprint() - base);
}
