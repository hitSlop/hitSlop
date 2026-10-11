import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addRow, kinds, removeRow, setAxis, setChart, setTitle, updateRow } from "./lab";

const row = s.string({ description: "The row's $id." });
const axis = s.enum(["x", "y"] as const, { description: "Which axis: x is horizontal, y is vertical." });

export default defineCommands(doc, {
  setTitle: {
    description: "Set the figure's title (up to 60 characters).",
    args: { title: s.string() },
    run: ({ tx }, { title }) => setTitle(tx, title),
  },
  setChart: {
    description: "Choose the chart kind and switch the least squares fit and the error bars on or off; omitted settings stay as they are.",
    args: {
      kind: s.optional(s.enum(kinds)),
      fit: s.optional(s.boolean({ description: "Draw an ordinary least squares line (scatter and line only)." })),
      errorBars: s.optional(s.boolean({ description: "Draw plus-or-minus bars from each row's uncertainty." })),
    },
    run: ({ tx }, change) => setChart(tx, change),
  },
  setAxis: {
    description: "Name an axis and its unit; omitted values stay as they are.",
    args: { axis, label: s.optional(s.string()), unit: s.optional(s.string()) },
    run: ({ tx }, { axis, ...change }) => setAxis(tx, axis, change),
  },
  addRow: {
    description: "Add a measurement (at most 60 rows) and return its stable row ID; x continues the previous spacing when omitted.",
    args: {
      x: s.optional(s.number()),
      y: s.optional(s.number()),
      err: s.optional(s.number({ min: 0, description: "Uncertainty in y, plus or minus." })),
      label: s.optional(s.string({ description: "Category name for bar charts." })),
    },
    run: ({ tx, current }, input) => addRow(tx, current, input),
  },
  updateRow: {
    description: "Change any of a measurement's x, y, uncertainty or label.",
    args: {
      row,
      x: s.optional(s.number()),
      y: s.optional(s.number()),
      err: s.optional(s.number({ min: 0 })),
      label: s.optional(s.string()),
    },
    run: ({ tx }, { row, ...change }) => updateRow(tx, row, change),
  },
  removeRow: {
    description: "Remove a measurement from the table.",
    args: { row },
    run: ({ tx }, { row }) => removeRow(tx, row),
  },
});
