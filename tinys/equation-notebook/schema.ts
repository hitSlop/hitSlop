import { defineDocument, s } from "@hitslop/tinys";

export const kinds = ["equation", "note", "graph"] as const;
export type Kind = (typeof kinds)[number];
export const maxLatex = 2000;
export const paramLimit = 1000;
export const windowLimit = 10000;

export default defineDocument({
  title: s.text({ description: "The problem being worked, shown in the title block." }),
  cells: s.list(s.object({
    kind: s.enum(kinds, { description: "equation: one typeset line; note: words; graph: a large plot of one function." }),
    latex: s.string({ maxLength: maxLatex, default: "", description: "The line as LaTeX (equation and graph cells). Only LaTeX is stored." }),
    note: s.text({ description: "The words of a note cell." }),
    plot: s.boolean({ default: false, description: "An equation cell shows its own graph. Graph cells always plot." }),
    params: s.record(s.number({ min: -paramLimit, max: paramLimit }), {
      description: "Slider values for the single letters other than x, y and e in the LaTeX. A letter with no entry reads as 1.",
    }),
    xMin: s.number({ min: -windowLimit, max: windowLimit, default: -10, description: "Left edge of the graph." }),
    xMax: s.number({ min: -windowLimit, max: windowLimit, default: 10, description: "Right edge of the graph." }),
  })),
}, { initial: {
  title: "Where does the parabola cross zero?",
  cells: [
    { kind: "note", note: "Find where y = x² − 2x − 3 meets the x-axis, then check it on the graph." },
    { kind: "equation", latex: "y=x^2-2x-3", plot: true, xMin: -4, xMax: 6 },
    { kind: "note", note: "Set y to zero and factor." },
    { kind: "equation", latex: "x^2-2x-3=0" },
    { kind: "equation", latex: "\\left(x-3\\right)\\left(x+1\\right)=0" },
    { kind: "equation", latex: "x=3\\quad\\text{or}\\quad x=-1" },
    { kind: "note", note: "Check with the quadratic formula, a = 1, b = −2, c = −3:" },
    { kind: "equation", latex: "x=\\frac{2\\pm\\sqrt{4+12}}{2}=\\frac{2\\pm4}{2}" },
    { kind: "note", note: "In vertex form the same curve is a(x − h)² + k. Move the sliders to see how the roots shift." },
    { kind: "graph", latex: "y=a\\left(x-h\\right)^2+k", params: { a: 1, h: 1, k: -4 }, xMin: -4, xMax: 6 },
  ],
} });
