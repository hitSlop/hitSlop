import { defineDocument, s } from "@hitslop/tinys";
import { kinds } from "./lab";

const text = (max: number, description: string) => s.string({ maxLength: max, default: "", description });

export default defineDocument({
  title: s.string({ maxLength: 60, default: "" }),
  kind: s.enum(kinds, { default: "scatter" }),
  xLabel: text(32, "Name of the horizontal quantity, e.g. Load."),
  xUnit: text(12, "Unit of the horizontal quantity, e.g. N."),
  yLabel: text(32, "Name of the vertical quantity, e.g. Extension."),
  yUnit: text(12, "Unit of the vertical quantity, e.g. mm."),
  fit: s.boolean({ default: false, description: "Draw an ordinary least squares line (scatter and line charts)." }),
  errorBars: s.boolean({ default: false, description: "Draw plus-or-minus bars from each row's uncertainty." }),
  rows: s.list(s.object({
    label: text(24, "Optional category name; bar charts use it, falling back to x."),
    x: s.number({ default: 0 }),
    y: s.number({ default: 0 }),
    err: s.number({ min: 0, default: 0, description: "Uncertainty in y, plus or minus. Zero draws no bar." }),
  })),
}, { initial: {
  title: "Spring extension under load",
  kind: "scatter",
  xLabel: "Load", xUnit: "N",
  yLabel: "Extension", yUnit: "mm",
  fit: true,
  errorBars: true,
  rows: [
    { label: "", x: 0.5, y: 6.3, err: 1.4 },
    { label: "", x: 1.0, y: 12.4, err: 1.4 },
    { label: "", x: 1.5, y: 17.8, err: 1.4 },
    { label: "", x: 2.0, y: 24.5, err: 1.4 },
    { label: "", x: 2.5, y: 30.1, err: 1.4 },
    { label: "", x: 3.0, y: 35.6, err: 1.4 },
    { label: "", x: 3.5, y: 42.9, err: 1.4 },
    { label: "", x: 4.0, y: 47.8, err: 1.4 },
  ],
} });
