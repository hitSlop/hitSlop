//! Native saved-view rendering. Called only after the child installs its sandbox.
use base64::{Engine, engine::general_purpose::STANDARD};
use rquickjs::{CatchResultExt, Function};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use takumi::prelude::*;
use takumi::unstable::base::viewport::MediaTarget;

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
    let sources = resource_images(request)?;
    if frames.len() != 3
        || frames.iter().any(|f| f.width == 0 || f.height == 0 || f.width > 512 || f.height > 512 || f.kind.is_some())
    {
        return Err("Widget batch must contain three bounded canvases".into());
    }
    render(frames, fonts, sources, 2, crate::WIDGET_OUTPUT, false).map_err(|e| format!("Widget rendering failed: {e}"))
}

pub fn evaluate_view(
    bundle: &str,
    request: &str,
    pdf: bool,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<String, String> {
    let Batch { frames, fonts } = evaluate_markup(bundle, request, "__slopCapture", interrupted)?;
    let sources = resource_images(request)?;
    if frames.len() != 1 {
        return Err("Capture requires one canvas".into());
    }
    let frame = &frames[0];
    let icon = frame.kind.as_deref() == Some("icon");
    if (pdf && frame.kind.as_deref() != Some("export"))
        || (icon && (frame.width != 512 || frame.height != 512))
        || !matches!(frame.kind.as_deref(), Some("icon" | "preview" | "export"))
        || frame.width == 0
        || frame.height == 0
        || frame.width > 16384
        || frame.height > 16384
    {
        return Err("Invalid capture canvas".into());
    }
    render(
        frames,
        fonts,
        sources,
        if icon || pdf { 1 } else { 2 },
        if pdf { crate::PDF_OUTPUT } else { crate::CAPTURE_OUTPUT },
        pdf,
    )
    .map_err(|e| format!("Capture rendering failed: {e}"))
}

/// Decode owner-supplied saved image bytes once, inside the restricted child.
/// Takumi's browser-like missing-image behavior must not silently replace useful
/// prior artwork when an attachment is damaged.
fn resource_images(request: &str) -> std::result::Result<HashMap<Arc<str>, ImageSource>, String> {
    #[derive(Deserialize)]
    struct Resources<'a> {
        #[serde(default, borrow)]
        resources: HashMap<&'a str, &'a str>,
    }
    let request: Resources<'_> = serde_json::from_str(request).map_err(|e| e.to_string())?;
    let mut images = HashMap::new();
    for (id, url) in request.resources {
        let (header, bytes) = url.split_once(',').ok_or_else(|| format!("Invalid image resource {id}"))?;
        if !matches!(
            header,
            "data:image/png;base64" | "data:image/jpeg;base64" | "data:image/gif;base64" | "data:image/webp;base64"
        ) {
            return Err(format!("Unsupported image resource {id}"));
        }
        let bytes = STANDARD.decode(bytes).map_err(|e| format!("Invalid image resource {id}: {e}"))?;
        let source = ImageSource::from_bytes(&bytes).map_err(|e| format!("Cannot decode image resource {id}: {e}"))?;
        images.insert(Arc::from(format!("slop-attachment:{id}")), source);
    }
    Ok(images)
}

fn evaluate_markup(
    bundle: &str,
    request: &str,
    entry: &str,
    interrupted: impl FnMut() -> bool + 'static,
) -> std::result::Result<Batch, String> {
    let (runtime, context) = crate::realm::create(interrupted)?;
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
    sources: HashMap<Arc<str>, ImageSource>,
    scale: u32,
    output_limit: usize,
    pdf: bool,
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
            viewport.media_target = if pdf { MediaTarget::Print } else { MediaTarget::Screen };
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
                    .images(sources.clone())
                    .node(node.clone())
                    .stylesheet(stylesheet.clone())
                    .build(),
            )?;
            if !measured.height.is_finite() || measured.height <= 0.0 || measured.height > 1_000_000.0 * scale as f32 {
                return Err("Export content height is empty, non-finite or exceeds 1,000,000 CSS pixels".into());
            }
            height = (measured.height / scale as f32).ceil() as u32;
            let right = measured_right(&measured);
            if !right.is_finite() || right > if pdf { 1_000_000.0 } else { 16384.0 } {
                return Err("Export exceeds the content width limit".into());
            }
            width = width.max((right / scale as f32).ceil() as u32);
            if frame.kind.as_deref() == Some("preview") {
                height = height.min(frame.height.max(width * 3));
            }
        }
        if pdf {
            // Upstream PDF maps renderer pixels to 0.75 points. A fractional DPR
            // keeps ordinary output at one point per CSS pixel and uniformly
            // scales long documents within the PDF page limit without a fork.
            let dpr = (14_400.0 / width.max(height) as f32).min(1.0) / 0.75;
            let pixels = |side: u32| ((side as f32 * dpr).round() as u32).clamp(1, 19_200);
            let mut pdf_viewport = Viewport::new((pixels(width), pixels(height))).with_device_pixel_ratio(dpr);
            pdf_viewport.media_target = MediaTarget::Print;
            let reference = pdf_viewport.unit_reference.get_or_insert_default();
            reference.width = frame.width as f32 * dpr;
            reference.height = frame.height as f32 * dpr;
            let bytes = takumi_pdf::render(
                takumi_pdf::PdfOptions::builder()
                    .viewport(pdf_viewport)
                    .fonts(&fonts)
                    .images(sources.clone())
                    .node(node)
                    .stylesheet(stylesheet)
                    .build(),
            )?;
            if bytes.len() > crate::PDF_BYTES {
                return Err("PDF exceeds its output byte limit".into());
            }
            images.push(STANDARD.encode(bytes));
            continue;
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
                .images(sources.clone())
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
    let output = serde_json::to_string(&crate::RenderReply { ok: true, images: Some(images), error: None })?;
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
