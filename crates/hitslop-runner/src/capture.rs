//! Native saved-view rendering. Called only after the child installs its sandbox.
use base64::{Engine, engine::general_purpose::STANDARD};
use rquickjs::{CatchResultExt, Context, Function, Runtime};
use serde::Deserialize;
use std::sync::Arc;
use takumi::prelude::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    html: String,
    css: String,
    width: u32,
    height: u32,
    #[serde(default)]
    kind: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Font {
    family: String,
    bytes: String,
    weight: Option<f32>,
    italic: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    frames: Vec<Frame>,
    fonts: Vec<Font>,
}

pub fn evaluate(
    bundle: &str,
    request: &str,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<String, String> {
    let Batch { frames, fonts } = evaluate_markup(bundle, request, "__slopWidget", interrupted)?;
    if frames.len() != 3
        || frames.iter().any(|f| f.width == 0 || f.height == 0 || f.width > 512 || f.height > 512 || f.kind.is_some())
    {
        return Err("Widget batch must contain three bounded canvases".into());
    }
    render(frames, fonts, 2, crate::WIDGET_OUTPUT).map_err(|e| format!("Widget rendering failed: {e}"))
}

pub fn evaluate_view(
    bundle: &str,
    request: &str,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<String, String> {
    let Batch { frames, fonts } = evaluate_markup(bundle, request, "__slopCapture", interrupted)?;
    if frames.len() != 1 {
        return Err("Capture requires one canvas".into());
    }
    let frame = &frames[0];
    let icon = frame.kind.as_deref() == Some("icon");
    if (icon && (frame.width != 512 || frame.height != 512))
        || !matches!(frame.kind.as_deref(), Some("icon" | "preview" | "export"))
        || frame.width == 0
        || frame.height == 0
        || frame.width > 16384
        || frame.height > 16384
    {
        return Err("Invalid capture canvas".into());
    }
    render(frames, fonts, if icon { 1 } else { 2 }, crate::CAPTURE_OUTPUT)
        .map_err(|e| format!("Capture rendering failed: {e}"))
}

fn evaluate_markup(
    bundle: &str,
    request: &str,
    entry: &str,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<Batch, String> {
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(256 << 20);
    runtime.set_max_stack_size(512 << 10);
    runtime.set_interrupt_handler(Some(Box::new(interrupted)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;
    let json = context.with(|ctx| {
        ctx.eval::<(), _>(bundle.as_bytes()).catch(&ctx).map_err(|e| e.to_string())?;
        let render: Function = ctx.globals().get(entry).catch(&ctx).map_err(|e| e.to_string())?;
        render.call::<_, String>((request,)).catch(&ctx).map_err(|e| e.to_string())
    })?;
    // SSR is finished. Release its heap before font registration and rasterization.
    drop(context);
    drop(runtime);
    if json.len() > 16 << 20 {
        return Err("Capture markup exceeds 16 MiB".into());
    }
    let Batch { frames, fonts } = serde_json::from_str(&json).map_err(|e| format!("Invalid capture markup: {e}"))?;
    if fonts.len() > 64 {
        return Err("Capture uses more than 64 font faces".into());
    }
    Ok(Batch { frames, fonts })
}

fn render(
    frames: Vec<Frame>,
    packaged_fonts: Vec<Font>,
    scale: u32,
    output_limit: usize,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let mut fonts = Fonts::default();
    fonts.register(
        FontResource::new(include_bytes!("../fonts/Geist.woff2").as_slice()).generic_family(GenericFamily::SANS_SERIF),
    )?;
    fonts.register(
        FontResource::new(include_bytes!("../fonts/Newsreader.ttf").as_slice()).generic_family(GenericFamily::SERIF),
    )?;
    fonts.register(
        FontResource::new(include_bytes!("../fonts/GeistMono.woff2").as_slice())
            .generic_family(GenericFamily::MONOSPACE),
    )?;
    // Converted CBDT/CBLC with GSUB sequences; no platform font discovery is needed.
    fonts.register(
        FontResource::new(include_bytes!("../fonts/AppleColorEmoji.ttf").as_slice())
            .generic_family(GenericFamily::EMOJI),
    )?;
    // Text symbols such as U+2713 are outside Geist and the emoji presentation
    // font. Register after Apple emoji so colored emoji retain their chosen face.
    fonts.register(FontResource::new(include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf").as_slice()))?;
    for font in packaged_fonts {
        if font.family.is_empty()
            || font.family.len() > 256
            || font.weight.is_some_and(|weight| !weight.is_finite() || !(1.0..=1000.0).contains(&weight))
        {
            return Err("Invalid packaged font metadata".into());
        }
        fonts.register(FontResource::new(STANDARD.decode(font.bytes)?).override_info(FontOverride {
            family_name: Some(font.family.into()),
            weight: font.weight,
            style: Some(if font.italic { FontStyle::italic() } else { FontStyle::normal() }),
            ..Default::default()
        }))?;
    }
    let mut images = Vec::with_capacity(3);
    for frame in frames {
        let natural_size = matches!(frame.kind.as_deref(), Some("preview" | "export"));
        let node = Node::from_html(&frame.html, FromHtmlOptions::default())?;
        // Shared authored CSS can include browser-only declarations for the editor.
        // Match Takumi's HTML/JS entry points: ignore unsupported declarations,
        // retaining the supported rules instead of refusing the entire stylesheet.
        // Keep layout anchored to the saved viewport while the output canvas grows
        // around overflow. Reflowing at each measured width makes right-aligned
        // overflow move again and can never converge.
        let css =
            if natural_size { format!("{}\nhtml{{width:{}px!important}}", frame.css, frame.width) } else { frame.css };
        let stylesheet = Arc::new(StyleSheet::parse_loosy(&css));
        let viewport = |width: u32, height: Option<u32>| {
            let mut viewport =
                Viewport::new((Some(width * scale), height.map(|h| h * scale))).with_device_pixel_ratio(scale as f32);
            // Unbounded layout still resolves vh against the saved window, just as
            // the browser capture does before extending its full-content canvas.
            let reference = viewport.unit_reference.get_or_insert_default();
            reference.width = (frame.width * scale) as f32;
            reference.height = (frame.height * scale) as f32;
            viewport
        };
        let mut width = frame.width;
        let mut height = frame.height;
        if natural_size {
            let measured = takumi::measure(
                RenderOptions::builder()
                    .viewport(viewport(width, None))
                    .fonts(&fonts)
                    .node(node.clone())
                    .stylesheet(stylesheet.clone())
                    .build(),
            )?;
            if !measured.height.is_finite() || measured.height <= 0.0 {
                return Err("Export has no measurable content".into());
            }
            height = (measured.height / scale as f32).ceil() as u32;
            let right = measured_right(&measured);
            if !right.is_finite() || right > 16384.0 {
                return Err("Export exceeds the image width limit".into());
            }
            width = width.max((right / scale as f32).ceil() as u32);
            if frame.kind.as_deref() == Some("preview") {
                height = height.min(frame.height.max(width * 3));
            }
        }
        if width as u64 * scale as u64 > 16384
            || height as u64 * scale as u64 > 16384
            || width as u64 * height as u64 * scale as u64 * scale as u64 > 24_000_000
        {
            return Err(
                "PNG exceeds 16384 pixels per side or 24 megapixels at 2×; export as PDF for longer documents".into()
            );
        }
        let image = takumi::render(
            RenderOptions::builder()
                .viewport(viewport(width, Some(height)))
                .fonts(&fonts)
                .node(node)
                .stylesheet(stylesheet)
                .build(),
        )?;
        let mut png = Vec::new();
        // The shared image contract accepts RGB/RGBA, not Takumi's automatic
        // palette optimization for flat artwork. Encode those same pixels directly.
        let mut encoder = png::Encoder::new(&mut png, image.width(), image.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(image.as_raw())?;
        images.push(STANDARD.encode(png));
    }
    let output = serde_json::json!({"ok": true, "images": images}).to_string();
    if output.len() > output_limit {
        return Err("Capture output exceeds its byte limit".into());
    }
    Ok(output)
}

fn measured_right(node: &MeasuredNode) -> f32 {
    let [a, _, c, _, x, _] = node.transform;
    let right = x + (a * node.width).max(0.0) + (c * node.height).max(0.0);
    node.children.iter().fold(right, |right, child| right.max(measured_right(child)))
}
