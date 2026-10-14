import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { tints } from "./schema";
import { rangeKeys, setColumnWidth, stampCells, tintCells, writeCells } from "./sheet";

const range = s.string({ description: "A cell like B2 or a range like B2:C4. Columns A-F, rows 1-12." });

export default defineCommands(schema, {
  setCell: {
    description: "Type into one cell (a number, words, or a formula like =SUM(B2:B5)) and return the cell.",
    args: { cell: s.string({ description: "The cell, like B2." }), input: s.string({ description: "What to type; empty clears it." }) },
    run: ({ tx, current }, { cell, input }) => {
      const [key] = rangeKeys(cell.trim());
      writeCells(tx, current, [[key!, input]]);
      return key;
    },
  },
  clearCells: {
    description: "Clear the typed contents of a cell or range and return how many cells it covered.",
    args: { range },
    run: ({ tx, current }, args) => writeCells(tx, current, rangeKeys(args.range).map(key => [key, ""] as const)),
  },
  tintCells: {
    description: "Color a cell or range with a crayon, or erase the color when no crayon is given. Returns the cell count.",
    args: { range, tint: s.optional(s.enum(tints, { description: "The crayon; omit to erase." })) },
    run: ({ tx, current }, args) => {
      const keys = rangeKeys(args.range);
      tintCells(tx, current, keys, args.tint ?? null);
      return keys.length;
    },
  },
  stampCells: {
    description: "Stamp a cell or range with an emoji, or remove the stamp when none is given. Returns the cell count.",
    args: { range, stamp: s.optional(s.string({ description: "An emoji such as 🍕; omit to remove." })) },
    run: ({ tx, current }, args) => stampCells(tx, current, rangeKeys(args.range), args.stamp ?? null),
  },
  setColumnWidth: {
    description: "Set a column's width in pixels (56 to 320), or restore the default when no width is given.",
    args: { column: s.string({ description: "The column letter, A-F." }), width: s.optional(s.integer({ min: 56, max: 320 })) },
    run: ({ tx, current }, args) => setColumnWidth(tx, current, args.column, args.width ?? null),
  },
});
