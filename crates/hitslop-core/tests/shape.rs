//! Window silhouettes: one parser for native and authoring. Oracle: independently
//! computed geometry and the SVG/CSS grammar, not another implementation.
use hitslop_core::shape::{Length, Segment, Silhouette, silhouette};
use serde_json::json;

fn path(source: &str) -> Result<Vec<Segment>, String> {
    match silhouette(Some(&json!({"path": source})), 100.0, 100.0) {
        Ok(Silhouette::Path { segments, .. }) => Ok(segments),
        Ok(other) => panic!("expected a path, got {other:?}"),
        Err(error) => Err(error.code.as_str().to_owned()),
    }
}
fn radius(value: &str) -> Result<([Length; 4], [Length; 4]), String> {
    match silhouette(Some(&json!(value)), 100.0, 100.0) {
        Ok(Silhouette::Radii { horizontal, vertical }) => Ok((horizontal, vertical)),
        Ok(other) => panic!("expected radii, got {other:?}"),
        Err(error) => Err(error.code.as_str().to_owned()),
    }
}
const PX: fn(f64) -> Length = |value| Length { value, percent: false };
const PC: fn(f64) -> Length = |value| Length { value, percent: true };
fn end(segment: &Segment) -> (f64, f64) {
    match *segment {
        Segment::Move { x, y } | Segment::Line { x, y } | Segment::Cubic { x, y, .. } => (x, y),
        Segment::Close => panic!("close has no end point"),
    }
}

#[test]
fn radius_grammar_follows_css_border_radius() {
    for valid in [
        "0",
        "22px",
        "9999px",
        "50%",
        "1px 2% 3px",
        "48px 8px 72px 0 / 24px 32px 18px 0",
        " .5px / 20% 30% ",
        "5.0px",
        ".5%",
    ] {
        assert!(radius(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "",
        "-1px",
        "1em",
        "1",
        "0 0 0 0 0",
        "1px /",
        "1px / 2px / 3px",
        "calc(10px)",
        "var(--radius)",
        "Infinitypx",
        "1e3px",
        ".px",
        "1.2.3px",
        "+1px",
        "5.px",
        "5.%",
    ] {
        assert_eq!(radius(invalid), Err("invalid_shape".into()), "{invalid}");
    }
    // One to four values expand in CSS order; a single axis applies to both.
    assert_eq!(
        radius("1px 2% 3px").unwrap(),
        ([PX(1.0), PC(2.0), PX(3.0), PC(2.0)], [PX(1.0), PC(2.0), PX(3.0), PC(2.0)])
    );
    assert_eq!(radius("4px 6px / 50%").unwrap(), ([PX(4.0), PX(6.0), PX(4.0), PX(6.0)], [PC(50.0); 4]));
}

#[test]
fn omitted_shape_is_the_default_rounded_window() {
    assert_eq!(
        silhouette(None, 480.0, 620.0).unwrap(),
        Silhouette::Radii { horizontal: [PX(22.0); 4], vertical: [PX(22.0); 4] }
    );
}

#[test]
fn path_grammar_accepts_the_svg_forms_and_refuses_malformed_input_whole() {
    for valid in [
        "M0 0H100V100H0Z",
        "m10 10 30 0 0 30-30 0z",
        "M.5.5L1e2-2e1 20,30Z",
        "M0 0C10 0 20 30 40 40S70 20 80 40Q90 60 100 40T120 40Z",
        "m10 10c10 0 20 30 40 40s30-20 40 0q10 20 20 0t20 0z",
        "M0 0A20 10 30 0110 20L0 20Z",
        "M0 0 A-20 -10 30 1 0 40 40Z",
        "M0 0A0 10 0 0 1 20 20Z",
        "M0 0A10 10 0 0 1 0 0Z",
        "M0 0L40 0L40 40Z m10 10h5v5h-5z",
        "M50 0a50 50 0 1 0 0 100a50 50 0 1 0 0-100Z",
    ] {
        assert!(path(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "",
        "Z",
        "L0 0",
        "M",
        "M0",
        "M,0 0",
        "M0,,0",
        "M0 0,",
        "M0 0,Z",
        "M0 0L1",
        "M0 0A1 1 0 2 0 5 5",
        "M0 0A1 1 0 0 +1 5 5",
        "M0 0A1 1 0 0 1",
        "M0 0Z 1 1",
        "M0 0R1 2",
        "M0 0L1e 2",
        "M0 0L1e309 0",
        "M0 0L1e308 0",
        "M0 0<script>",
    ] {
        assert_eq!(path(invalid), Err("invalid_shape".into()), "{invalid}");
    }
}

#[test]
fn paths_normalize_to_absolute_lines_and_cubics() {
    // Implicit linetos after a relative moveto, and relative H/V.
    assert_eq!(
        path("m10 10 30 0 0 30-30 0z").unwrap(),
        vec![
            Segment::Move { x: 10.0, y: 10.0 },
            Segment::Line { x: 40.0, y: 10.0 },
            Segment::Line { x: 40.0, y: 40.0 },
            Segment::Line { x: 10.0, y: 40.0 },
            Segment::Close,
        ]
    );
    // A quadratic becomes the cubic with controls two thirds of the way to its control.
    assert_eq!(
        path("M0 0Q30 60 90 0").unwrap()[1],
        Segment::Cubic { x1: 20.0, y1: 40.0, x2: 50.0, y2: 40.0, x: 90.0, y: 0.0 }
    );
    // S reflects the previous cubic's second control; T the previous quadratic's control.
    let smooth = path("M0 0C10 0 20 10 30 10S50 20 60 10").unwrap();
    assert!(matches!(smooth[2], Segment::Cubic { x1, y1, .. } if (x1, y1) == (40.0, 10.0)));
    let chained = path("M0 0Q10 10 20 0T40 0").unwrap();
    assert_eq!(
        chained[2],
        Segment::Cubic {
            x1: 20.0 + 2.0 / 3.0 * 10.0,
            y1: 2.0 / 3.0 * -10.0,
            x2: 40.0 + 2.0 / 3.0 * -10.0,
            y2: 2.0 / 3.0 * -10.0,
            x: 40.0,
            y: 0.0
        }
    );
    // Z returns to the subpath start, so relative commands continue from there.
    assert_eq!(path("M10 10l10 0z l5 0").unwrap().last(), Some(&Segment::Line { x: 15.0, y: 10.0 }));
}

#[test]
fn arcs_become_quarter_turn_cubics_that_end_exactly_at_the_endpoint() {
    // A full circle from two half-circle arcs: four quarter cubics, radius 50 about (50, 50).
    let circle = path("M50 0a50 50 0 1 0 0 100a50 50 0 1 0 0-100Z").unwrap();
    let cubics: Vec<_> = circle.iter().filter(|s| matches!(s, Segment::Cubic { .. })).collect();
    assert_eq!(cubics.len(), 4);
    for segment in &cubics {
        let (x, y) = end(segment);
        assert!(((x - 50.0).hypot(y - 50.0) - 50.0).abs() < 1e-9, "{segment:?}");
    }
    assert_eq!(end(cubics[1]), (50.0, 100.0));
    // Radii too small for the chord scale up (F.6.6); zero radii draw a line; a zero
    // length arc draws nothing.
    let corrected = path("M0 50A1 1 0 0 1 100 50").unwrap();
    assert_eq!(end(corrected.last().unwrap()), (100.0, 50.0));
    assert_eq!(path("M0 0A0 10 0 0 1 20 20").unwrap()[1], Segment::Line { x: 20.0, y: 20.0 });
    assert_eq!(path("M0 0A10 10 0 0 1 0 0").unwrap(), vec![Segment::Move { x: 0.0, y: 0.0 }]);
}

#[test]
fn path_options_and_resource_limits() {
    let shape = json!({"path": "M0 0H10V10Z", "viewBox": [20, 40], "fillRule": "evenodd"});
    assert!(matches!(
        silhouette(Some(&shape), 100.0, 100.0).unwrap(),
        Silhouette::Path { view_box: [20.0, 40.0], even_odd: true, .. }
    ));
    // The view box defaults to the window's logical size.
    let default = json!({"path": "M0 0H10V10Z"});
    assert!(matches!(
        silhouette(Some(&default), 480.0, 620.0).unwrap(),
        Silhouette::Path { view_box: [480.0, 620.0], even_odd: false, .. }
    ));
    for bad in [
        json!({"path": "M0 0Z", "viewBox": [0, 10]}),
        json!({"path": "M0 0Z", "fillRule": "inherit"}),
        json!({"path": "M0 0Z", "extra": 1}),
        json!(3),
    ] {
        assert!(silhouette(Some(&bad), 100.0, 100.0).is_err(), "{bad}");
    }
    // At most 512 commands (an arc expands to at most four cubics, so output stays
    // bounded) and 4,096 bytes of source.
    let lines = |n: usize| format!("M0 0{}", "L1 1".repeat(n - 1));
    assert!(path(&lines(512)).is_ok());
    assert!(path(&lines(513)).is_err());
    assert!(path(&format!("M0 0{}", " ".repeat(4092))).is_ok());
    assert!(path(&format!("M0 0{}", " ".repeat(4093))).is_err());
}
