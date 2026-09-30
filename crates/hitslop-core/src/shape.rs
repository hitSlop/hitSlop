//! Window silhouettes: the manifest's radius grammar and bounded SVG path data, parsed
//! and normalized once. Native builds its path from the output; authoring validates
//! through WASM. This module is independent of Loro and of document semantics.
use crate::{err, Code, Error, Result};
use serde::Deserialize;
use serde_json::Value;

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
#[derive(Clone, Debug, PartialEq)]
pub enum Silhouette {
    /// CSS `border-radius` order: top-left, top-right, bottom-right, bottom-left.
    Radii { horizontal: [Length; 4], vertical: [Length; 4] },
    /// `view_box` is the coordinate space the segments are drawn in.
    Path { segments: Vec<Segment>, view_box: [f64; 2], even_odd: bool },
}

const MAX_SOURCE: usize = 4096;
/// Arcs expand to at most four cubics, so output stays under 2,048 segments.
const MAX_COMMANDS: usize = 512;

fn invalid() -> Error {
    err(Code::InvalidShape, "Invalid window silhouette")
}
/// Rejects values whose later geometry (scaled up to 16,384 points) cannot be finite.
fn finite(values: &[f64]) -> Result<()> {
    if values.iter().all(|v| v.is_finite() && (v * 16384.0).is_finite()) { Ok(()) } else { Err(invalid()) }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathShape {
    path: String,
    #[serde(rename = "viewBox")]
    view_box: Option<[f64; 2]>,
    #[serde(rename = "fillRule")]
    fill_rule: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Shape {
    Radius(String),
    Path(PathShape),
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

fn normalize(shape: Shape, width: f64, height: f64) -> Result<Silhouette> {
    match shape {
        Shape::Radius(value) => radii(&value),
        Shape::Path(shape) => {
            let view_box = shape.view_box.unwrap_or([width, height]);
            if !view_box.iter().all(|n| n.is_finite() && (1.0..=16384.0).contains(n)) {
                return Err(invalid());
            }
            let even_odd = match shape.fill_rule.as_deref() {
                None | Some("nonzero") => false,
                Some("evenodd") => true,
                Some(_) => return Err(invalid()),
            };
            Ok(Silhouette::Path { segments: Parser::new(&shape.path)?.parse()?, view_box, even_odd })
        }
    }
}

/// One to four lengths per axis, optionally `horizontal / vertical`, as in CSS.
fn radii(input: &str) -> Result<Silhouette> {
    if input.len() > 256 || !input.bytes().all(|b| b"0123456789.px%/ \t\r\n".contains(&b)) {
        return Err(invalid());
    }
    let halves: Vec<&str> = input.split('/').collect();
    if !(1..=2).contains(&halves.len()) {
        return Err(invalid());
    }
    let side = |text: &str| -> Result<[Length; 4]> {
        let items = text.split_ascii_whitespace().map(length).collect::<Result<Vec<_>>>()?;
        Ok(match items.as_slice() {
            [a] => [*a, *a, *a, *a],
            [a, b] => [*a, *b, *a, *b],
            [a, b, c] => [*a, *b, *c, *b],
            [a, b, c, d] => [*a, *b, *c, *d],
            _ => return Err(invalid()),
        })
    };
    let horizontal = side(halves[0])?;
    let vertical = if halves.len() == 2 { side(halves[1])? } else { horizontal };
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
    if (whole.is_empty() && fraction.is_empty()) || !digits(whole) || !digits(fraction)
        || (whole.is_empty() && !number.starts_with('.'))
        || (number.contains('.') && fraction.is_empty())
    {
        return Err(invalid());
    }
    let value: f64 = number.parse().map_err(|_| invalid())?;
    finite(&[value])?;
    Ok(Length { value, percent })
}

/// Bounded SVG path data. Malformed input is refused whole: no partial prefix is drawn.
struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    /// A number was just read, so a comma may separate the next one.
    numeric: bool,
    x: f64,
    y: f64,
    start: (f64, f64),
    /// The previous command's last control point, for S/T reflection.
    control: (f64, f64),
    commands: usize,
    out: Vec<Segment>,
}
impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self> {
        let bytes = source.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_SOURCE
            || !bytes.iter().all(|b| b"MmLlHhVvCcSsQqTtAaZz0123456789eE+.,- \t\r\n".contains(b))
        {
            return Err(invalid());
        }
        Ok(Self { bytes, at: 0, numeric: false, x: 0.0, y: 0.0, start: (0.0, 0.0), control: (0.0, 0.0), commands: 0, out: vec![] })
    }
    fn space(&mut self) {
        while self.bytes.get(self.at).is_some_and(|b| b" \t\r\n".contains(b)) {
            self.at += 1;
        }
    }
    fn digits(&mut self) -> usize {
        let from = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        self.at - from
    }
    fn number(&mut self, flag: bool) -> Result<f64> {
        self.space();
        if self.bytes.get(self.at) == Some(&b',') {
            if !self.numeric {
                return Err(invalid());
            }
            self.at += 1;
            self.space();
        }
        let from = self.at;
        if flag {
            // Arc flags are a single 0 or 1 and may be written without separators.
            if !matches!(self.bytes.get(self.at), Some(b'0' | b'1')) {
                return Err(invalid());
            }
            self.at += 1;
        } else {
            if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            let mut count = self.digits();
            if self.bytes.get(self.at) == Some(&b'.') {
                self.at += 1;
                count += self.digits();
            }
            if count == 0 {
                return Err(invalid());
            }
            if matches!(self.bytes.get(self.at), Some(b'e' | b'E')) {
                self.at += 1;
                if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                    self.at += 1;
                }
                if self.digits() == 0 {
                    return Err(invalid());
                }
            }
        }
        let text = std::str::from_utf8(&self.bytes[from..self.at]).map_err(|_| invalid())?;
        let value: f64 = text.parse().map_err(|_| invalid())?;
        finite(&[value])?;
        self.numeric = true;
        Ok(value)
    }
    fn point(&mut self, relative: bool) -> Result<(f64, f64)> {
        let (dx, dy) = if relative { (self.x, self.y) } else { (0.0, 0.0) };
        let px = self.number(false)? + dx;
        let py = self.number(false)? + dy;
        finite(&[px, py])?;
        Ok((px, py))
    }
    fn cubic(&mut self, c1: (f64, f64), c2: (f64, f64), end: (f64, f64)) -> Result<()> {
        finite(&[c1.0, c1.1, c2.0, c2.1, end.0, end.1])?;
        self.out.push(Segment::Cubic { x1: c1.0, y1: c1.1, x2: c2.0, y2: c2.1, x: end.0, y: end.1 });
        Ok(())
    }
    fn parse(mut self) -> Result<Vec<Segment>> {
        let (mut command, mut previous) = (0u8, 0u8);
        loop {
            self.space();
            let Some(&next) = self.bytes.get(self.at) else { break };
            if next.is_ascii_alphabetic() {
                command = next;
                self.at += 1;
                self.numeric = false;
            } else if command == 0 {
                return Err(invalid());
            }
            let relative = command.is_ascii_lowercase();
            let upper = command.to_ascii_uppercase();
            if (self.commands == 0 && upper != b'M') || self.commands >= MAX_COMMANDS {
                return Err(invalid());
            }
            self.commands += 1;
            match upper {
                b'Z' => {
                    self.out.push(Segment::Close);
                    (self.x, self.y) = self.start;
                    command = 0;
                    self.numeric = false;
                }
                b'M' | b'L' => {
                    let (px, py) = self.point(relative)?;
                    (self.x, self.y) = (px, py);
                    if upper == b'M' {
                        self.out.push(Segment::Move { x: px, y: py });
                        self.start = (px, py);
                        // Coordinate pairs after a moveto are implicit linetos.
                        command = if relative { b'l' } else { b'L' };
                    } else {
                        self.out.push(Segment::Line { x: px, y: py });
                    }
                }
                b'H' | b'V' => {
                    let n = self.number(false)?;
                    let base = if upper == b'H' { self.x } else { self.y };
                    let value = n + if relative { base } else { 0.0 };
                    if upper == b'H' { self.x = value } else { self.y = value }
                    finite(&[self.x, self.y])?;
                    self.out.push(Segment::Line { x: self.x, y: self.y });
                }
                b'C' | b'S' => {
                    let first = if upper == b'C' {
                        self.point(relative)?
                    } else if matches!(previous, b'C' | b'S') {
                        (2.0 * self.x - self.control.0, 2.0 * self.y - self.control.1)
                    } else {
                        (self.x, self.y)
                    };
                    let second = self.point(relative)?;
                    let end = self.point(relative)?;
                    self.cubic(first, second, end)?;
                    self.control = second;
                    (self.x, self.y) = end;
                }
                b'Q' | b'T' => {
                    let control = if upper == b'Q' {
                        self.point(relative)?
                    } else if matches!(previous, b'Q' | b'T') {
                        (2.0 * self.x - self.control.0, 2.0 * self.y - self.control.1)
                    } else {
                        (self.x, self.y)
                    };
                    let end = self.point(relative)?;
                    // A quadratic is exactly the cubic with controls 2/3 of the way to it.
                    let c1 = (self.x + 2.0 / 3.0 * (control.0 - self.x), self.y + 2.0 / 3.0 * (control.1 - self.y));
                    let c2 = (end.0 + 2.0 / 3.0 * (control.0 - end.0), end.1 + 2.0 / 3.0 * (control.1 - end.1));
                    self.cubic(c1, c2, end)?;
                    self.control = control;
                    (self.x, self.y) = end;
                }
                b'A' => {
                    let rx = self.number(false)?.abs();
                    let ry = self.number(false)?.abs();
                    let rotation = self.number(false)?;
                    let large = self.number(true)? == 1.0;
                    let sweep = self.number(true)? == 1.0;
                    let end = self.point(relative)?;
                    self.arc(rx, ry, rotation, large, sweep, end)?;
                    (self.x, self.y) = end;
                }
                _ => return Err(invalid()),
            }
            previous = upper;
        }
        if self.commands == 0 {
            return Err(invalid());
        }
        Ok(self.out)
    }
    /// SVG arc implementation notes (F.6.5–F.6.6): radii correction, then at most
    /// quarter-turn cubic approximations.
    fn arc(&mut self, rx: f64, ry: f64, rotation: f64, large: bool, sweep: bool, end: (f64, f64)) -> Result<()> {
        if end == (self.x, self.y) {
            return Ok(());
        }
        if rx == 0.0 || ry == 0.0 {
            self.out.push(Segment::Line { x: end.0, y: end.1 });
            return Ok(());
        }
        let (mut rx, mut ry) = (rx, ry);
        let angle = (rotation % 360.0).to_radians();
        let (s, c) = angle.sin_cos();
        let (dx, dy) = ((self.x - end.0) / 2.0, (self.y - end.1) / 2.0);
        let (xp, yp) = (c * dx + s * dy, -s * dx + c * dy);
        let (mut ux, mut uy) = (xp / rx, yp / ry);
        let length = ux.hypot(uy);
        finite(&[xp, yp, length])?;
        if length > 1.0 {
            rx *= length;
            ry *= length;
            (ux, uy) = (xp / rx, yp / ry);
        }
        let norm = ux * ux + uy * uy;
        let k = if large == sweep { -1.0 } else { 1.0 } * ((1.0 - norm) / norm).max(0.0).sqrt();
        let (cxp, cyp) = (k * rx * uy, -k * ry * ux);
        let center = (c * cxp - s * cyp + (self.x + end.0) / 2.0, s * cxp + c * cyp + (self.y + end.1) / 2.0);
        let (ax, ay) = ((xp - cxp) / rx, (yp - cyp) / ry);
        let (bx, by) = ((-xp - cxp) / rx, (-yp - cyp) / ry);
        let start = ay.atan2(ax);
        let mut delta = (ax * by - ay * bx).atan2(ax * bx + ay * by);
        if !sweep && delta > 0.0 {
            delta -= std::f64::consts::TAU;
        }
        if sweep && delta < 0.0 {
            delta += std::f64::consts::TAU;
        }
        finite(&[rx, ry, center.0, center.1, start, delta])?;
        let count = ((delta.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
        let step = delta / count as f64;
        let position = |t: f64| (center.0 + c * rx * t.cos() - s * ry * t.sin(), center.1 + s * rx * t.cos() + c * ry * t.sin());
        let derivative = |t: f64| (-c * rx * t.sin() - s * ry * t.cos(), -s * rx * t.sin() + c * ry * t.cos());
        let alpha = 4.0 / 3.0 * (step / 4.0).tan();
        for index in 0..count {
            let a = start + index as f64 * step;
            let b = a + step;
            let (p, q, dp, dq) = (position(a), position(b), derivative(a), derivative(b));
            // The last segment ends exactly at the authored endpoint.
            let to = if index == count - 1 { end } else { q };
            self.cubic((p.0 + alpha * dp.0, p.1 + alpha * dp.1), (q.0 - alpha * dq.0, q.1 - alpha * dq.1), to)?;
        }
        Ok(())
    }
}
