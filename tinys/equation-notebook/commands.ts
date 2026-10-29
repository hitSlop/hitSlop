import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { kinds } from "./schema";
import { addCell, setLatex, setNote, setParam, setPlot, setWindow } from "./notebook";

const cell = s.string({ description: "The cell's $id." });

export default defineCommands(schema, {
  addCell: {
    description: "Append an equation, note or graph cell (or insert it after another cell) and return its $id.",
    args: {
      kind: s.enum(kinds),
      latex: s.optional(s.string({ description: "LaTeX for an equation or graph cell, such as y=x^2-1." })),
      note: s.optional(s.string({ description: "Words for a note cell." })),
      plot: s.optional(s.boolean({ description: "Show the equation's own graph." })),
      after: s.optional(s.string({ description: "Insert after this cell's $id instead of at the end." })),
    },
    run: ({ tx }, { kind, latex, note, plot, after }) => addCell(tx, kind, { latex, note, plot, after }),
  },
  setLatex: {
    description: "Replace the LaTeX of an equation or graph cell.",
    args: { cell, latex: s.string() },
    run: ({ tx, current }, args) => setLatex(tx, current, args.cell, args.latex),
  },
  setNote: {
    description: "Replace the words of a note cell.",
    args: { cell, text: s.string() },
    run: ({ tx, current }, args) => setNote(tx, current, args.cell, args.text),
  },
  setPlot: {
    description: "Show or hide an equation cell's own graph.",
    args: { cell, plot: s.boolean() },
    run: ({ tx, current }, args) => setPlot(tx, current, args.cell, args.plot),
  },
  setParam: {
    description: "Set the slider value of a one-letter parameter (not x, y or e) in a cell's equation.",
    args: { cell, name: s.string(), value: s.number() },
    run: ({ tx, current }, args) => setParam(tx, current, args.cell, args.name, args.value),
  },
  setWindow: {
    description: "Set the left and right edges of a cell's graph; the height fits the curve.",
    args: { cell, xMin: s.number(), xMax: s.number() },
    run: ({ tx, current }, args) => setWindow(tx, current, args.cell, args.xMin, args.xMax),
  },
  removeCell: {
    description: "Remove a cell from the notebook.",
    args: { cell },
    run: ({ tx }, args) => tx.fields.cells.remove(args.cell),
  },
});
