#![cfg(feature = "capture")]
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use tinys_runner::{Evaluator, JobKind};

fn render(evaluator: &Evaluator, bundle: &str, mode: JobKind) -> Result<String, String> {
    match evaluator.markup(1, bundle, "{}", matches!(mode, JobKind::Widget)) {
        Ok(batch) => evaluator.raster(1, batch, Default::default(), mode),
        Err(error) => Ok(json!({"ok":false,"error":error}).to_string()),
    }
}
fn run(batch: Value) -> Value {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
    let bundle = format!("globalThis.__tinyWidget = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
    serde_json::from_str(&render(&evaluator, &bundle, JobKind::Widget).unwrap()).unwrap()
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
fn capture_reads_future_batch_frame_and_font_hints_openly() {
    let mut input = batch();
    input["hint"] = json!("future");
    input["frames"][0]["hint"] = json!({"future":true});
    input["fonts"] = json!([{"family":"Authored Name","italic":false,"weight":400,
        "bytes":STANDARD.encode(include_bytes!("../../../tests/apps/assets/kalam.woff2")),"hint":true}]);
    let result = run(input);
    assert_eq!(result["ok"], true, "{result}");
}

#[test]
fn saved_view_capture_measures_exports_crops_previews_and_bounds_rasters() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
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
            format!("globalThis.__tinyCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
        let reply: Value = serde_json::from_str(&render(&evaluator, &bundle, JobKind::Capture).unwrap()).unwrap();
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
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
    let batch = json!({"fonts":[],"frames":[{"kind":"export","width":200,"height":100,
        "html":"<html><body><div></div></body></html>",
        "css":"html,body{margin:0;width:100%;height:100px}body{position:relative}div{position:absolute;left:100%;width:20px;height:100px;background:#123456}"}]});
    let bundle = format!("globalThis.__tinyCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
    let reply: Value = serde_json::from_str(&render(&evaluator, &bundle, JobKind::Capture).unwrap()).unwrap();
    assert_eq!(reply["ok"], true, "{reply}");
    let bytes = STANDARD.decode(reply["images"][0].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (440, 200));
    assert_eq!(&pixels[400 * 4..401 * 4], &[0x12, 0x34, 0x56, 255]);
}

#[test]
fn vector_pdf_keeps_font_text_and_uniformly_bounds_long_pages() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
    for (height, expected) in [(200, [0.0, 0.0, 200.0, 200.0]), (20_000, [0.0, 0.0, 144.0, 14_400.0])] {
        let batch = json!({"fonts":[],"frames":[{"kind":"export","width":200,"height":100,
            "html":"<html><body><div>Selectable text 🌿<a href='#end'>Jump</a></div><div id='end'>Last line</div></body></html>",
            "css":format!("html,body{{margin:0;padding:0;width:200px;height:80px;font-family:sans-serif}}@media print{{html,body{{height:{height}px}}}}")}]});
        let bundle =
            format!("globalThis.__tinyCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
        let reply: Value = serde_json::from_str(&render(&evaluator, &bundle, JobKind::Pdf).unwrap()).unwrap();
        assert_eq!(reply["ok"], true, "{reply}");
        let bytes = STANDARD.decode(reply["images"][0].as_str().unwrap()).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let pdf = String::from_utf8_lossy(&bytes);
        let media_box = pdf
            .split("/MediaBox")
            .nth(1)
            .expect("a PDF page has bounds")
            .trim_start()
            .strip_prefix('[')
            .unwrap()
            .split(']')
            .next()
            .unwrap();
        let bounds: Vec<f32> = media_box.split_whitespace().map(|number| number.parse().unwrap()).collect();
        assert_eq!(bounds.len(), 4);
        // Upstream PDF viewports use integer pixels, so point dimensions can round
        // by a fraction of a point while retaining the page's proportions.
        for (actual, expected) in bounds.iter().zip(expected) {
            assert!((actual - expected).abs() < 0.5, "wrong point bounds: {bounds:?}");
        }
        assert!(bounds[2] <= 14_400.0 && bounds[3] <= 14_400.0, "page exceeds PDF bounds: {bounds:?}");
        assert!(pdf.contains("/ToUnicode"), "vector text needs a Unicode map for selection");
        assert!(pdf.contains("/FontFile"), "text uses embedded font subsets");
    }
}

#[test]
fn damaged_saved_image_refuses_capture_instead_of_publishing_a_blank() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
    let url = format!("data:image/png;base64,{}", STANDARD.encode(b"damaged image"));
    let request = json!({"saved-image":url});
    let batch = json!({"fonts":[],"frames":[{"kind":"export","width":200,"height":100,
        "html":"<html><body><img src='tiny-attachment:saved-image' width='100' height='100'></body></html>",
        "css":"html,body{margin:0;width:200px;height:100px}"}]});
    let bundle = format!("globalThis.__tinyCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
    for mode in [JobKind::Capture, JobKind::Pdf] {
        let markup = evaluator.markup(1, &bundle, "{}", false).unwrap();
        let result = evaluator.raster(1, markup, serde_json::from_value(request.clone()).unwrap(), mode);
        let reply: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(reply["ok"], false);
        assert!(reply["error"].as_str().unwrap().contains("Cannot decode image resource saved-image"));
        assert!(reply.get("images").is_none());
    }
}

#[test]
fn repeated_attachment_tokens_share_saved_bytes_and_render_each_occurrence() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&[0x12, 0x34, 0x56, 255]).unwrap();
    let request = json!({"saved-image":format!("data:image/png;base64,{}", STANDARD.encode(png))});
    let batch = json!({"fonts":[],"frames":[{"kind":"export","width":100,"height":40,
        "html":"<html><body><img src='tiny-attachment:saved-image'><img src='tiny-attachment:saved-image'></body></html>",
        "css":"html,body{margin:0;padding:0;width:100px;height:40px}body{display:flex}img{width:50px;height:40px}"}]});
    let bundle = format!("globalThis.__tinyCapture = () => {};", serde_json::to_string(&batch.to_string()).unwrap());
    let reply: Value = serde_json::from_str(
        &evaluator
            .raster(
                1,
                evaluator.markup(1, &bundle, "{}", false).unwrap(),
                serde_json::from_value(request).unwrap(),
                JobKind::Capture,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(reply["ok"], true, "{reply}");
    let bytes = STANDARD.decode(reply["images"][0].as_str().unwrap()).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (200, 80));
    for x in [25, 175] {
        let offset = (40 * 200 + x) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0x12, 0x34, 0x56, 255]);
    }
}

#[test]
fn markup_runs_without_image_bytes_and_raster_refuses_javascript() {
    let evaluator = Evaluator::new(env!("CARGO_BIN_EXE_tinys-evaluator").into(), vec![]).unwrap();
    let bundle = "globalThis.__tinyCapture = json => { const r=JSON.parse(json); if(r.attachments.photo!== 'image/png' || 'resources' in r) throw Error('image bytes leaked'); return JSON.stringify({frames:[{kind:'preview',width:64,height:64,html:'<html>Saved</html>',css:''}],fonts:[]}); };";
    let markup = evaluator.markup(1, bundle, r#"{"attachments":{"photo":"image/png"}}"#, false).unwrap();
    assert_eq!(markup.frames.len(), 1);
    let output = evaluator.raster(1, markup.clone(), Default::default(), JobKind::Capture).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&output).unwrap()["ok"], true);
    for field in ["bundle", "request"] {
        let mut job =
            json!({"runtimeABI":1,"bundle":"","request":"","mode":"capture","raster":{"batch":markup,"images":{}}});
        job[field] = json!("globalThis.__tinyCapture = () => {throw Error('executed')}");
        let error = tinys_runner::evaluate(&serde_json::to_vec(&job).unwrap(), || false).unwrap_err();
        assert!(error.contains("cannot carry JavaScript"), "{error}");
    }
    let job = json!({"runtimeABI":1,"bundle":"","request":"","mode":"capture","raster":{"batch":markup,"images":{},"javascript":"throw Error('executed')"}});
    assert!(
        tinys_runner::evaluate(&serde_json::to_vec(&job).unwrap(), || false).unwrap_err().contains("unknown field")
    );
}
