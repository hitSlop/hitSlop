import type { ShapeSegment } from "../wire/app.generated";

/** Draws Rust's normalized silhouette using SVG's absolute path commands. */
export function silhouettePath(segments: ShapeSegment[]): string {
  return segments.map(segment => {
    switch (segment.kind) {
      case "move": return `M${segment.x} ${segment.y}`;
      case "line": return `L${segment.x} ${segment.y}`;
      case "cubic": return `C${segment.x1} ${segment.y1} ${segment.x2} ${segment.y2} ${segment.x} ${segment.y}`;
      case "close": return "Z";
    }
  }).join(" ");
}
