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
fn radius(value: &str) -> Result<(Vec<Length>, Vec<Length>), String> {
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
        (vec![PX(1.0), PC(2.0), PX(3.0), PC(2.0)], vec![PX(1.0), PC(2.0), PX(3.0), PC(2.0)])
    );
    assert_eq!(radius("4px 6px / 50%").unwrap(), (vec![PX(4.0), PX(6.0), PX(4.0), PX(6.0)], vec![PC(50.0); 4]));
}

#[test]
fn omitted_shape_is_the_default_rounded_window() {
    assert_eq!(
        silhouette(None, 480.0, 620.0).unwrap(),
        Silhouette::Radii { horizontal: vec![PX(22.0); 4], vertical: vec![PX(22.0); 4] }
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
        "M0 0, \tZ",
        "M0 0, \n",
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
fn arcs_follow_the_circle_and_end_exactly_at_the_endpoint() {
    // A full circle from two half-circle arcs, radius 50 about (50, 50).
    let circle = path("M50 0a50 50 0 1 0 0 100a50 50 0 1 0 0-100Z").unwrap();
    let cubics: Vec<_> = circle.iter().filter(|s| matches!(s, Segment::Cubic { .. })).collect();
    for segment in &cubics {
        let (x, y) = end(segment);
        assert!(((x - 50.0).hypot(y - 50.0) - 50.0).abs() < 1e-9, "{segment:?}");
    }
    assert!(cubics.iter().any(|s| end(s) == (50.0, 100.0)));
    assert_eq!(end(cubics.last().unwrap()), (50.0, 0.0));
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
        Silhouette::Path { view_box_width: 20.0, view_box_height: 40.0, even_odd: true, .. }
    ));
    // The view box defaults to the window's logical size.
    let default = json!({"path": "M0 0H10V10Z"});
    assert!(matches!(
        silhouette(Some(&default), 480.0, 620.0).unwrap(),
        Silhouette::Path { view_box_width: 480.0, view_box_height: 620.0, even_odd: false, .. }
    ));
    for bad in [
        json!({"path": "M0 0Z", "viewBox": [0, 10]}),
        json!({"path": "M0 0Z", "fillRule": "inherit"}),
        json!({"path": "M0 0Z", "extra": 1}),
        json!(3),
    ] {
        assert!(silhouette(Some(&bad), 100.0, 100.0).is_err(), "{bad}");
    }
    // At most 512 source commands and 4,096 bytes of source.
    let lines = |n: usize| format!("M0 0{}", "L1 1".repeat(n - 1));
    assert!(path(&lines(512)).is_ok());
    assert!(path(&lines(513)).is_err());
    assert!(path(&format!("M0 0{}", " ".repeat(4092))).is_ok());
    assert!(path(&format!("M0 0{}", " ".repeat(4093))).is_err());
}

#[test]
fn arcs_obey_the_rendered_tolerance_at_different_view_box_scales() {
    for size in [100.0, 1000.0, 16_384.0] {
        let shape = json!({"path":"M50 0a50 50 0 1 0 0 100a50 50 0 1 0 0-100Z", "viewBox":[100,100]});
        let Silhouette::Path { segments, .. } = silhouette(Some(&shape), size, size).unwrap() else { panic!() };
        let mut from = (50.0, 0.0);
        for segment in segments {
            if let Segment::Cubic { x1, y1, x2, y2, x, y } = segment {
                for step in 0..=32 {
                    let t = step as f64 / 32.0;
                    let u = 1.0 - t;
                    let sample = |a, b, c, d| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
                    let px = sample(from.0, x1, x2, x);
                    let py = sample(from.1, y1, y2, y);
                    let error = ((px - 50.0).hypot(py - 50.0) - 50.0).abs() * size / 100.0;
                    assert!(error <= 0.1, "rendered radial error {error} at size {size}");
                }
                from = (x, y);
            }
        }
    }
}

#[test]
fn extreme_arcs_are_refused_without_partial_geometry_or_panics() {
    for source in [
        "M0 0A1e22 1e22 0 0 1 2e22 0",
        "M0 0A1e100 1e100 0 1 1 2e100 0",
        "M0 0A1e300 1e300 0 1 1 1 0",
        "M0 0A1e-6 1e-6 0 0 1 100 0",
        "M0 0A1 1 0 0 1 1e-200 0",
    ] {
        assert_eq!(path(source), Err("invalid_shape".into()), "{source}");
    }
}

#[test]
fn rotated_ellipses_preserve_all_four_arc_choices() {
    // Start and end are a quarter-turn apart on a 30×15 ellipse rotated 30 degrees.
    // The other possible ellipse center is the sum of those two radius vectors.
    let rotation = std::f64::consts::PI / 6.0;
    let (sin, cos) = rotation.sin_cos();
    let from = (40.0 + 30.0 * cos, 50.0 + 30.0 * sin);
    let to = (40.0 - 15.0 * sin, 50.0 + 15.0 * cos);
    for large in [false, true] {
        for sweep in [false, true] {
            let source =
                format!("M{} {}A30 15 30 {} {} {} {}", from.0, from.1, u8::from(large), u8::from(sweep), to.0, to.1);
            let center = if large == sweep {
                (40.0 + 30.0 * cos - 15.0 * sin, 50.0 + 30.0 * sin + 15.0 * cos)
            } else {
                (40.0, 50.0)
            };
            let segments = path(&source).unwrap();
            let mut start = from;
            let mut angle = 0.0;
            for segment in &segments {
                if let Segment::Cubic { x1, y1, x2, y2, x, y } = *segment {
                    let local = |p: (f64, f64)| {
                        let (x, y) = (p.0 - center.0, p.1 - center.1);
                        ((x * cos + y * sin) / 30.0, (-x * sin + y * cos) / 15.0)
                    };
                    let mut previous = local(start);
                    for step in 1..=32 {
                        let t = step as f64 / 32.0;
                        let u = 1.0 - t;
                        let sample =
                            |a, b, c, d| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
                        let p = local((sample(start.0, x1, x2, x), sample(start.1, y1, y2, y)));
                        assert!((p.0.hypot(p.1) - 1.0).abs() * 30.0 <= 0.1, "{source}");
                        angle += (previous.0 * p.1 - previous.1 * p.0).atan2(previous.0 * p.0 + previous.1 * p.1);
                        previous = p;
                    }
                    start = (x, y);
                }
            }
            assert_eq!(end(segments.last().unwrap()), to);
            let expected = std::f64::consts::FRAC_PI_2 * if large { 3.0 } else { 1.0 } * if sweep { 1.0 } else { -1.0 };
            assert!((angle - expected).abs() < 1e-9, "{source}: {angle}");
        }
    }
}
