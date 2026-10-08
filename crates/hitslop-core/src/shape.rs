//! Window silhouettes: the manifest's radius grammar and bounded SVG path data, parsed
//! and normalized once. Native builds its path from the output; authoring validates
//! through WASM. This module is independent of Loro and of document semantics.
use crate::{Code, Error, Result, err};
use kurbo::{Arc, PathEl, Point, SvgArc, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use svgtypes::{PathParser, PathSegment};

/// One corner length: points, or a percentage of the window's width or height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length {
    pub value: f64,
    pub percent: bool,
}
/// Absolute path commands in SVG's y-down coordinates. Lines, quadratics, arcs and the
/// shorthand forms are all expressed as lines and cubics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Move { x: f64, y: f64 },
    Line { x: f64, y: f64 },
    Cubic { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    Close,
}
/// A window's shape, as the host draws it (it crosses to Swift as it is).
#[derive(Clone, Debug, PartialEq)]
pub enum Silhouette {
    /// Four lengths per axis, in CSS `border-radius` order: top-left, top-right,
    /// bottom-right, bottom-left.
    Radii { horizontal: Vec<Length>, vertical: Vec<Length> },
    /// The view box is the coordinate space the segments are drawn in.
    Path { segments: Vec<Segment>, view_box_width: f64, view_box_height: f64, even_odd: bool },
}

use crate::wire::{SHAPE_PATH, SHAPE_RADIUS, SHAPE_VIEW_BOX, present_option};
const MAX_COMMANDS: usize = 512;
const MAX_SEGMENTS: usize = 2048;
/// Approximation target in logical points at the declared window size.
const ARC_TOLERANCE: f64 = 0.1;

fn invalid() -> Error {
    err(Code::InvalidShape, "Invalid window silhouette")
}
/// Rejects values whose later geometry (scaled up to 16,384 points) cannot be finite.
fn finite(values: &[f64]) -> Result<()> {
    if values.iter().all(|v| v.is_finite() && (v * SHAPE_VIEW_BOX).is_finite()) { Ok(()) } else { Err(invalid()) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "app.generated.ts"))]
pub struct PathShape {
    pub path: String,
    #[serde(rename = "viewBox", default, deserialize_with = "present_option", skip_serializing_if = "Option::is_none")]
    pub view_box: Option<[f64; 2]>,
    #[serde(
        rename = "fillRule",
        default,
        deserialize_with = "present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub fill_rule: Option<FillRule>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum FillRule {
    Nonzero,
    Evenodd,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "WindowShape", export_to = "app.generated.ts"))]
pub enum Shape {
    Radius(String),
    Path(PathShape),
}

/// Checks a manifest `shape` given as JSON text (`None` for the default rounded corners),
/// as authoring validates it.
pub fn validate(shape: Option<&str>, width: f64, height: f64) -> Result<()> {
    let shape: Option<Value> = shape.map(serde_json::from_str).transpose().map_err(|_| invalid())?;
    silhouette(shape.as_ref(), width, height).map(|_| ())
}
/// The silhouette for a manifest `shape` (or `None` for the default rounded corners) on a
/// window of `width` × `height` points.
pub fn silhouette(shape: Option<&Value>, width: f64, height: f64) -> Result<Silhouette> {
    let shape = match shape {
        None => Shape::Radius(crate::wire::DEFAULT_WINDOW_RADIUS.into()),
        Some(value) => Shape::deserialize(value).map_err(|_| invalid())?,
    };
    normalize(shape, width, height)
}

pub(crate) fn normalize(shape: Shape, width: f64, height: f64) -> Result<Silhouette> {
    match shape {
        Shape::Radius(value) => radii(&value),
        Shape::Path(shape) => {
            let view_box = shape.view_box.unwrap_or([width, height]);
            if !view_box.iter().all(|n| n.is_finite() && (1.0..=SHAPE_VIEW_BOX).contains(n)) {
                return Err(invalid());
            }
            let even_odd = matches!(shape.fill_rule, Some(FillRule::Evenodd));
            let [view_box_width, view_box_height] = view_box;
            Ok(Silhouette::Path {
                segments: path(&shape.path, ARC_TOLERANCE / (width / view_box_width).max(height / view_box_height))?,
                view_box_width,
                view_box_height,
                even_odd,
            })
        }
    }
}

/// One to four lengths per axis, optionally `horizontal / vertical`, as in CSS.
fn radii(input: &str) -> Result<Silhouette> {
    if input.len() > SHAPE_RADIUS || !input.bytes().all(|b| b"0123456789.px%/ \t\r\n".contains(&b)) {
        return Err(invalid());
    }
    let halves: Vec<&str> = input.split('/').collect();
    if !(1..=2).contains(&halves.len()) {
        return Err(invalid());
    }
    let side = |text: &str| -> Result<Vec<Length>> {
        let items = text.split_ascii_whitespace().map(length).collect::<Result<Vec<_>>>()?;
        Ok(match items.as_slice() {
            [a] => vec![*a, *a, *a, *a],
            [a, b] => vec![*a, *b, *a, *b],
            [a, b, c] => vec![*a, *b, *c, *b],
            [a, b, c, d] => vec![*a, *b, *c, *d],
            _ => return Err(invalid()),
        })
    };
    let horizontal = side(halves[0])?;
    let vertical = if halves.len() == 2 { side(halves[1])? } else { horizontal.clone() };
    Ok(Silhouette::Radii { horizontal, vertical })
}
/// `0`, or a nonnegative decimal with `px` or `%`. No exponents or other units.
fn length(token: &str) -> Result<Length> {
    if token == "0" {
        return Ok(Length { value: 0.0, percent: false });
    }
    let (number, percent) = match (token.strip_suffix("px"), token.strip_suffix('%')) {
        (Some(n), _) => (n, false),
        (_, Some(n)) => (n, true),
        _ => return Err(invalid()),
    };
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty())
        || !digits(whole)
        || !digits(fraction)
        || (whole.is_empty() && !number.starts_with('.'))
        || (number.contains('.') && fraction.is_empty())
    {
        return Err(invalid());
    }
    let value: f64 = number.parse().map_err(|_| invalid())?;
    finite(&[value])?;
    Ok(Length { value, percent })
}

/// The library accepts a comma before a command or EOF. Require a numeric token after
/// each comma; all other token grammar belongs to PathParser.
fn path(source: &str, tolerance: f64) -> Result<Vec<Segment>> {
    if source.is_empty()
        || source.len() > SHAPE_PATH
        || !source.is_ascii()
        || !tolerance.is_finite()
        || tolerance <= 0.0
        || source.split(',').skip(1).any(|tail| {
            !tail
                .trim_start_matches([' ', '\t', '\r', '\n'])
                .as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_digit() || b"+-.".contains(b))
        })
    {
        return Err(invalid());
    }
    let mut path = Normalizer { at: Point::ORIGIN, start: Point::ORIGIN, out: vec![] };
    let (mut previous, mut control) = (b' ', Point::ORIGIN);
    for (index, segment) in PathParser::from(source).enumerate() {
        let segment = segment.map_err(|_| invalid())?;
        if index >= MAX_COMMANDS || (index == 0 && !matches!(segment, PathSegment::MoveTo { .. })) {
            return Err(invalid());
        }
        let at = path.at;
        let point = |abs, x, y| -> Result<Point> {
            finite(&[x, y])?;
            let p = if abs { Point::new(x, y) } else { at + Vec2::new(x, y) };
            finite(&[p.x, p.y])?;
            Ok(p)
        };
        let reflected = at + (at - control);
        let command = segment.command().to_ascii_uppercase();
        match segment {
            PathSegment::MoveTo { abs, x, y } => {
                let p = point(abs, x, y)?;
                path.push(Segment::Move { x: p.x, y: p.y })?;
                path.at = p;
                path.start = p;
            }
            PathSegment::LineTo { abs, x, y } => path.line(point(abs, x, y)?)?,
            PathSegment::HorizontalLineTo { abs, x } => {
                let p = point(abs, x, if abs { at.y } else { 0.0 })?;
                path.line(p)?;
            }
            PathSegment::VerticalLineTo { abs, y } => {
                let p = point(abs, if abs { at.x } else { 0.0 }, y)?;
                path.line(p)?;
            }
            PathSegment::CurveTo { abs, x1, y1, x2, y2, x, y } => {
                control = point(abs, x2, y2)?;
                path.cubic(point(abs, x1, y1)?, control, point(abs, x, y)?)?;
            }
            PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                let first = if matches!(previous, b'C' | b'S') { reflected } else { at };
                control = point(abs, x2, y2)?;
                path.cubic(first, control, point(abs, x, y)?)?;
            }
            PathSegment::Quadratic { abs, x1, y1, x, y } => {
                control = point(abs, x1, y1)?;
                path.quadratic(control, point(abs, x, y)?)?;
            }
            PathSegment::SmoothQuadratic { abs, x, y } => {
                control = if matches!(previous, b'Q' | b'T') { reflected } else { at };
                path.quadratic(control, point(abs, x, y)?)?;
            }
            PathSegment::EllipticalArc { abs, rx, ry, x_axis_rotation, large_arc, sweep, x, y } => {
                finite(&[rx, ry, x_axis_rotation])?;
                path.arc(
                    SvgArc {
                        from: at,
                        to: point(abs, x, y)?,
                        radii: Vec2::new(rx.abs(), ry.abs()),
                        x_rotation: (x_axis_rotation % 360.0).to_radians(),
                        large_arc,
                        sweep,
                    },
                    tolerance,
                )?;
            }
            PathSegment::ClosePath { .. } => {
                path.push(Segment::Close)?;
                path.at = path.start;
            }
        }
        previous = command;
    }
    if path.out.is_empty() {
        return Err(invalid());
    }
    Ok(path.out)
}

/// Only geometry normalization remains here: syntax and arc mathematics belong to the
/// libraries. Every emitted segment is checked before it enters the bounded output.
struct Normalizer {
    at: Point,
    start: Point,
    out: Vec<Segment>,
}
impl Normalizer {
    fn push(&mut self, segment: Segment) -> Result<()> {
        if self.out.len() >= MAX_SEGMENTS {
            return Err(invalid());
        }
        match segment {
            Segment::Move { x, y } | Segment::Line { x, y } => finite(&[x, y])?,
            Segment::Cubic { x1, y1, x2, y2, x, y } => finite(&[x1, y1, x2, y2, x, y])?,
            Segment::Close => {}
        }
        self.out.push(segment);
        Ok(())
    }
    fn line(&mut self, end: Point) -> Result<()> {
        self.push(Segment::Line { x: end.x, y: end.y })?;
        self.at = end;
        Ok(())
    }
    fn cubic(&mut self, first: Point, second: Point, end: Point) -> Result<()> {
        self.push(Segment::Cubic { x1: first.x, y1: first.y, x2: second.x, y2: second.y, x: end.x, y: end.y })?;
        self.at = end;
        Ok(())
    }
    fn quadratic(&mut self, control: Point, end: Point) -> Result<()> {
        self.cubic(self.at + (control - self.at) * (2.0 / 3.0), end + (control - end) * (2.0 / 3.0), end)
    }
    fn arc(&mut self, svg: SvgArc, tolerance: f64) -> Result<()> {
        if svg.from == svg.to {
            return Ok(());
        }
        if svg.radii.x == 0.0 || svg.radii.y == 0.0 {
            return self.line(svg.to);
        }
        // Kurbo treats tiny radii as lines, even when SVG radii correction would draw
        // a visible arc. Refuse those inputs rather than silently changing their shape.
        // A subnormal chord can also underflow its endpoint-to-center calculation.
        if svg.radii.x.min(svg.radii.y) <= 1e-5 || (svg.to - svg.from).hypot() < 1e-100 {
            return Err(invalid());
        }
        let Some(arc) = Arc::from_svg_arc(&svg) else { return self.line(svg.to) };
        finite(&[arc.center.x, arc.center.y, arc.radii.x, arc.radii.y, arc.start_angle, arc.sweep_angle])?;
        let start = self.out.len();
        // Unlike SimplifyingPathParser, this iterator never buffers the expanded arc.
        // push refuses the first excess segment, stopping work as well as allocation.
        for segment in arc.append_iter(tolerance) {
            let PathEl::CurveTo(first, second, end) = segment else { return Err(invalid()) };
            self.cubic(first, second, end)?;
        }
        if self.out.len() == start {
            return Err(invalid());
        }
        // Avoid accumulated rounding at a junction with the next authored command.
        if let Some(Segment::Cubic { x, y, .. }) = self.out.last_mut() {
            (*x, *y) = (svg.to.x, svg.to.y);
        }
        self.at = svg.to;
        Ok(())
    }
}
