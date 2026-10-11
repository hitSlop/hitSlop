import * as Plot from "@observablehq/plot";
import type { ForecastDay } from "./fsrs";

const weekday = (day: string) => new Date(`${day}T12:00:00Z`).toLocaleDateString(undefined, { weekday: "narrow", timeZone: "UTC" });

/** Scale to the load, not the cap: a generous budget would flatten every bar, so the cap
 * line is drawn only when it is within reach of the busiest day. */
export function capInScale(days: readonly ForecastDay[], cap: number) {
  return cap <= Math.max(4, ...days.map(day => day.reviews)) * 1.6;
}

/** The load forecast as pencil-shaded bars on the form. Built from data, owned by the view. */
export function drawStrip(days: readonly ForecastDay[], cap: number, width: number, height: number) {
  const roomy = (width - 40) / Math.max(1, days.length) >= 27;
  const rows = days.flatMap((day, index) => {
    const label = `${index === 0 && roomy ? "now" : weekday(day.date)}\n${Number(day.date.slice(8))}`;
    return [
      { label, kind: "Reviews", cards: Math.max(0, day.reviews - day.newCards) },
      { label, kind: "New", cards: day.newCards },
    ];
  });
  const totals = days.map((day, index) => ({
    label: rows[index * 2].label,
    total: Math.round(day.reviews),
    over: day.overBudget,
  }));
  const capShown = capInScale(days, cap);
  const top = Math.max(capShown ? cap : 0, ...days.map(day => day.reviews), 4) * 1.22;
  const last = () => totals[totals.length - 1].label;
  return Plot.plot({
    width, height,
    marginTop: 18, marginBottom: 34, marginLeft: 6, marginRight: 34,
    style: { background: "transparent", color: "var(--slop-graphite)", fontFamily: "var(--printed)", fontSize: "12px", overflow: "visible" },
    x: { type: "band", domain: totals.map(entry => entry.label), padding: 0.22, tickSize: 0, label: null },
    y: { domain: [0, top], axis: null },
    color: { domain: ["Reviews", "New"], range: ["var(--slop-graphite)", "var(--slop-tint)"] },
    marks: [
      Plot.ruleY([0], { stroke: "var(--slop-form)", strokeWidth: 1.2 }),
      Plot.barY(rows, { x: "label", y: "cards", fill: "kind", stroke: "var(--slop-form)", strokeWidth: (d: (typeof rows)[number]) => d.kind === "New" ? 1 : 0, rx: 1.5 }),
      capShown ? Plot.ruleY([cap], { stroke: "var(--slop-form)", strokeDasharray: "4 3", strokeWidth: 1.2 }) : null,
      capShown ? Plot.text([cap], { x: last, y: d => d, text: () => String(cap), dx: 30, textAnchor: "end", fill: "var(--slop-form)", fontSize: 11 }) : null,
      Plot.text(totals, { x: "label", y: (d: (typeof totals)[number]) => d.total, text: (d: (typeof totals)[number]) => (d.over ? `${d.total}!` : String(d.total)), dy: -7, fill: (d: (typeof totals)[number]) => d.over ? "var(--slop-form)" : "var(--slop-pencil)", fontSize: 11, fontWeight: 600 }),
    ],
  });
}
