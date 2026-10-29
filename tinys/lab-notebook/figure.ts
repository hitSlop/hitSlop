import * as Plot from "@observablehq/plot";
import { notebookView, type Notebook } from "./lab";

/** Builds the chart as an SVG element from document data. The view owns and discards it. */
export function drawFigure(data: Notebook, width: number, height: number): SVGSVGElement | HTMLElement {
  const view = notebookView(data);
  const { points, fit } = view;
  const bar = data.kind === "bar";
  const categories = points.map((p, i) => p.label || String(p.x) || String(i));
  // Duplicate category names would merge bars; number the repeats.
  const seen = new Map<string, number>();
  const names = categories.map(name => {
    const count = (seen.get(name) ?? 0) + 1;
    seen.set(name, count);
    return count > 1 ? `${name} (${count})` : name;
  });
  const items = points.map((p, i) => ({ ...p, name: names[i], lo: p.y - p.err, hi: p.y + p.err }));
  const withBars = items.filter(p => data.errorBars && p.err > 0);
  const ink = "var(--tiny-ink)";
  const series = "var(--tiny-series)";
  const sorted = data.kind === "line" ? [...items].sort((a, b) => a.x - b.x) : items;
  const marks: Plot.Markish[] = [];
  if (bar) {
    marks.push(Plot.barY(items, { x: "name", y: "y", fill: series, fillOpacity: 0.85, insetLeft: 4, insetRight: 4 }));
    marks.push(Plot.ruleY([0], { stroke: ink }));
    marks.push(Plot.ruleX(withBars, { x: "name", y1: "lo", y2: "hi", stroke: ink, strokeWidth: 1.6, dx: 0 }));
  } else {
    if (data.kind === "line") marks.push(Plot.line(sorted, { x: "x", y: "y", stroke: series, strokeWidth: 2.2 }));
    marks.push(Plot.ruleX(withBars, { x: "x", y1: "lo", y2: "hi", stroke: ink, strokeWidth: 1.6 }));
    // Caps are short horizontal rules, a fixed fraction of the x range wide.
    const xs = items.map(p => p.x);
    const cap = ((Math.max(...xs) - Math.min(...xs)) || 1) * 0.012;
    for (const edge of ["lo", "hi"] as const) {
      marks.push(Plot.ruleY(withBars, { y: edge, x1: (p: (typeof items)[number]) => p.x - cap, x2: (p: (typeof items)[number]) => p.x + cap, stroke: ink, strokeWidth: 1.6 }));
    }
    if (fit && items.length) {
      const xs = items.map(p => p.x);
      const [a, b] = [Math.min(...xs), Math.max(...xs)];
      marks.push(Plot.line([[a, fit.slope * a + fit.intercept], [b, fit.slope * b + fit.intercept]], {
        stroke: "var(--tiny-fit)", strokeWidth: 2.4, strokeDasharray: "7 5",
      }));
    }
    marks.push(Plot.dot(items, { x: "x", y: "y", r: 4.6, fill: series, stroke: "var(--tiny-card)", strokeWidth: 1.5 }));
  }
  return Plot.plot({
    width, height,
    marginLeft: 58, marginBottom: 46, marginTop: 30, marginRight: 22,
    style: { background: "transparent", color: ink, fontFamily: "var(--mono)", fontSize: "12px", overflow: "visible" },
    x: bar
      ? { type: "band", label: view.xTitle || null, tickSize: 0, padding: 0.2 }
      : { label: view.xTitle || null, grid: true, inset: 14, nice: true },
    y: { label: view.yTitle ? `↑ ${view.yTitle}` : null, grid: true, nice: true, zero: bar },
    marks,
  });
}
