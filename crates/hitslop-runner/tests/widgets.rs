#![cfg(feature = "capture")]
use base64::{Engine, engine::general_purpose::STANDARD};
use hitslop_runner::Evaluator;
use serde_json::{Value, json};

fn run(batch: Value) -> Value {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_hitslop-evaluator").into(), vec![]).unwrap();
    let bundle = format!("globalThis.__slopWidget = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
    serde_json::from_str(&evaluator.widget(1, &bundle, "{}").unwrap()).unwrap()
}
fn batch() -> Value {
    json!({"fonts":[], "frames": vec![json!({"width":64,"height":64,
        "html":"<html><body>🌿</body></html>",
        "css":"html,body{margin:0;width:100%;height:100%;background:white;color:black;font-size:40px;font-family:sans-serif}"});3]})
}
#[test]
fn restricted_widget_batch_renders_color_emoji_at_two_times_scale() {
    let result = run(batch());
    assert_eq!(result["ok"], true, "{result}");
    let images = result["images"].as_array().unwrap();
    assert_eq!(images.len(), 3);
    let bytes = STANDARD.decode(images[0].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (128, 128));
    let channels = info.color_type.samples();
    assert!(
        pixels[..info.buffer_size()].chunks_exact(channels).any(|pixel| pixel[1] > pixel[0].saturating_add(20)),
        "the leaf must contain colored pixels, not a missing glyph or monochrome fallback"
    );
}
#[test]
fn widget_refuses_incomplete_oversized_and_damaged_font_inputs() {
    for kind in ["count", "dimensions", "font"] {
        let mut input = batch();
        match kind {
            "count" => {
                input["frames"].as_array_mut().unwrap().pop();
            }
            "dimensions" => input["frames"][0]["width"] = json!(100000),
            _ => input["fonts"] = json!([{"family":"Broken", "bytes":STANDARD.encode(b"not a font"),"italic":false}]),
        }
        let output = run(input);
        assert_eq!(output["ok"], false, "{kind}: {output}");
        assert!(output.get("images").is_none());
    }
}

#[test]
fn packaged_font_bytes_register_under_the_authored_family_alias() {
    let mut input = batch();
    for frame in input["frames"].as_array_mut().unwrap() {
        frame["html"] = json!("<html><body>Abc</body></html>");
        frame["css"] = json!("body{margin:0;font-size:30px;font-family:'Authored Name',sans-serif;color:black}");
    }
    let fallback = run(input.clone());
    input["fonts"] = json!([{"family":"Authored Name","italic":false,"weight":400,
        "bytes":STANDARD.encode(include_bytes!("../../../tests/apps/assets/kalam.woff2"))}]);
    let custom = run(input);
    assert_eq!(custom["ok"], true, "{custom}");
    assert_eq!(fallback["ok"], true, "{fallback}");
    assert_ne!(custom["images"][0], fallback["images"][0], "the supplied face must replace the generic fallback");
}

#[test]
fn saved_view_capture_measures_exports_crops_previews_and_bounds_rasters() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_hitslop-evaluator").into(), vec![]).unwrap();
    for (kind, height, content_width, expected) in [
        ("icon", 512, 512, Some((512, 512))),
        ("export", 1000, 200, Some((400, 2000))),
        ("preview", 1000, 200, Some((400, 1200))),
        ("export", 1000, 400, Some((800, 2000))),
        ("export", 100000, 200, None),
    ] {
        let width = if kind == "icon" { 512 } else { 200 };
        let batch = json!({"fonts":[],"frames":[{"kind":kind,"width":width,"height":if kind == "icon" {512} else {100},
            "html":"<html><body><div></div></body></html>",
            "css":format!("html,body{{margin:0;padding:0;width:{width}px;height:auto}}div{{width:{content_width}px;height:{height}px;background:#123456}}")}]});
        let bundle =
            format!("globalThis.__slopCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
        let reply: Value = serde_json::from_str(&evaluator.capture(1, &bundle, "{}").unwrap()).unwrap();
        if let Some(dimensions) = expected {
            assert_eq!(reply["ok"], true, "{kind}: {reply}");
            let bytes = STANDARD.decode(reply["images"][0].as_str().unwrap()).unwrap();
            let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
            let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
            let info = decoder.next_frame(&mut pixels).unwrap();
            assert_eq!((info.width, info.height), dimensions, "{kind}");
            assert_eq!(&pixels[..4], &[0x12, 0x34, 0x56, 255], "{kind}");
        } else {
            assert_eq!(reply["ok"], false);
            assert!(reply["error"].as_str().unwrap().contains("24 megapixels"));
            assert!(reply.get("images").is_none());
        }
    }
}

#[test]
fn capture_extends_canvas_without_reflowing_viewport_relative_overflow() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_hitslop-evaluator").into(), vec![]).unwrap();
    let batch = json!({"fonts":[],"frames":[{"kind":"export","width":200,"height":100,
        "html":"<html><body><div></div></body></html>",
        "css":"html,body{margin:0;width:100%;height:100px}body{position:relative}div{position:absolute;left:100%;width:20px;height:100px;background:#123456}"}]});
    let bundle = format!("globalThis.__slopCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
    let reply: Value = serde_json::from_str(&evaluator.capture(1, &bundle, "{}").unwrap()).unwrap();
    assert_eq!(reply["ok"], true, "{reply}");
    let bytes = STANDARD.decode(reply["images"][0].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (440, 200));
    assert_eq!(&pixels[400 * 4..401 * 4], &[0x12, 0x34, 0x56, 255]);
}
