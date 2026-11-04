use serde::{Deserialize, Serialize};
/// One corner length: points, or a percentage of the window's width or height.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ShapeLength", export_to = "app.generated.ts"))]
pub struct Length {
    pub value: f64,
    pub percent: bool,
}
/// Absolute path commands in SVG's y-down coordinates. Lines, quadratics, arcs and the
/// shorthand forms are all expressed as lines and cubics.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ShapeSegment", export_to = "app.generated.ts"))]
pub enum Segment {
    Move { x: f64, y: f64 },
    Line { x: f64, y: f64 },
    Cubic { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    Close,
}
/// A window's shape, as the host draws it (it crosses to Swift as it is).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "app.generated.ts"))]
pub enum Silhouette {
    /// Four lengths per axis, in CSS `border-radius` order: top-left, top-right,
    /// bottom-right, bottom-left.
    Radii { horizontal: Vec<Length>, vertical: Vec<Length> },
    /// The view box is the coordinate space the segments are drawn in.
    Path { segments: Vec<Segment>, view_box_width: f64, view_box_height: f64, even_odd: bool },
}

use crate::wire::present_option;

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
