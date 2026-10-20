/// URL namespaces are explicit: the page cannot fetch command programs or artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceRoute {
    App,
    Attachment,
}
#[derive(Clone, Debug)]
pub struct ResourceInfo {
    pub size: u64,
    pub media_type: String,
}
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "resources.generated.ts"))]
pub struct ResourceSpan {
    pub offset: u64,
    pub length: u64,
}
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "resources.generated.ts"))]
pub struct ResponsePlan {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub span: Option<ResourceSpan>,
}
/// Plan one HTTP resource response independently of the transport's chunk size.
/// Malformed or multiple ranges are ignored; a valid empty range is unsatisfiable.
pub fn resource_response(info: &ResourceInfo, route: ResourceRoute, range: Option<&str>) -> ResponsePlan {
    let whole = ResourceSpan { offset: 0, length: info.size };
    let mut plan = ResponsePlan {
        status: 200,
        headers: std::collections::HashMap::from([
            ("Content-Type".into(), info.media_type.clone()),
            ("Cache-Control".into(), "no-store".into()),
            ("Accept-Ranges".into(), "bytes".into()),
            ("X-Content-Type-Options".into(), "nosniff".into()),
        ]),
        span: Some(whole),
    };
    if route == ResourceRoute::Attachment {
        plan.headers.insert("Content-Security-Policy".into(), "sandbox".into());
    }
    let bounds = range.and_then(|header| {
        let (first, last) = header.trim().strip_prefix("bytes=")?.split_once('-')?;
        let (first, last) = (first.trim(), last.trim());
        let number =
            |s: &str| (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u64>().ok()).flatten();
        if first.is_empty() {
            let suffix = number(last)?;
            Some(if suffix == 0 || info.size == 0 {
                None
            } else {
                Some(ResourceSpan { offset: info.size.saturating_sub(suffix), length: suffix.min(info.size) })
            })
        } else {
            let start = number(first)?;
            let end = if last.is_empty() { info.size.saturating_sub(1) } else { number(last)? };
            Some(if start >= info.size || end < start {
                None
            } else {
                Some(ResourceSpan { offset: start, length: end.min(info.size - 1) - start + 1 })
            })
        }
    });
    if let Some(span) = bounds {
        plan.status = if span.is_some() { 206 } else { 416 };
        let content_range = match &span {
            Some(span) => format!("bytes {}-{}/{}", span.offset, span.offset + span.length - 1, info.size),
            None => format!("bytes */{}", info.size),
        };
        plan.headers.insert("Content-Range".into(), content_range);
        plan.span = span;
    }
    plan.headers.insert("Content-Length".into(), plan.span.as_ref().map_or(0, |s| s.length).to_string());
    plan
}
