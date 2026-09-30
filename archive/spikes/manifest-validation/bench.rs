use std::{hint::black_box, time::Instant};
use serde_json::{json, Value};
use hitslop_core::manifest_spike::probe;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cases: Vec<Value> = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let input = cases[0]["input"].as_str().unwrap();
    let start = Instant::now();
    let cold = black_box(probe(black_box(input)));
    let cold_us = start.elapsed().as_secs_f64() * 1e6;
    let start = Instant::now();
    for _ in 0..1000 { black_box(probe(black_box(input))); }
    let warm_us = start.elapsed().as_secs_f64() * 1e6 / 1000.0;
    let results: Vec<Value> = cases.iter().map(|case| {
        json!({"name": case["name"], "result": serde_json::from_str::<Value>(&probe(case["input"].as_str().unwrap())).unwrap()})
    }).collect();
    println!("{}", json!({"cold_us": cold_us, "warm_us": warm_us, "cold_result": cold, "results": results}));
}
