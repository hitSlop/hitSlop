import type { ShapeSegment, Silhouette } from "../wire/app.generated";

/** Draws Rust's normalized silhouette using SVG's absolute path commands. */
function silhouettePath(segments: ShapeSegment[]): string {
  return segments
    .map((segment) => {
      switch (segment.kind) {
        case "move":
          return `M${segment.x} ${segment.y}`;
        case "line":
          return `L${segment.x} ${segment.y}`;
        case "cubic":
          return `C${segment.x1} ${segment.y1} ${segment.x2} ${segment.y2} ${segment.x} ${segment.y}`;
        case "close":
          return "Z";
      }
    })
    .join(" ");
}

/** A path silhouette as hidden SVG markup for `clip-path: url(#window-shape)`, scaled to the
 * clipped box. It holds numbers only, so it needs no escaping. */
export function silhouetteClip(shape: Extract<Silhouette, { kind: "path" }>): string {
  const rule = shape.evenOdd ? "evenodd" : "nonzero";
  return `<svg width="0" height="0" aria-hidden="true" style="position:absolute"><clipPath id="window-shape" clipPathUnits="objectBoundingBox"><path d="${silhouettePath(shape.segments)}" transform="scale(${1 / shape.viewBoxWidth},${1 / shape.viewBoxHeight})" clip-rule="${rule}"/></clipPath></svg>`;
}
