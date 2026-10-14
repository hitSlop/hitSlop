import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema from "./schema";
import { fillCells, paintCells, selectColor, setPalette } from "./sprite";

export default defineCommands(schema, {
  paintCell: {
    description: "Paint one cell (index 0-255, row by row) with a #rrggbb color, or erase it with an empty color; returns the number of cells changed.",
    args: { index: s.integer(), color: s.string({ description: "A #rrggbb color, or empty to erase." }) },
    run: ({ tx, current }, { index, color }) => paintCells(tx, current, [[index, color]]),
  },
  fill: {
    description: "Flood-fill the connected same-colored region around a cell and return the number of cells recolored.",
    args: { index: s.integer(), color: s.string({ description: "A #rrggbb color, or empty to erase." }) },
    run: ({ tx, current }, { index, color }) => fillCells(tx, current, index, color),
  },
  setPalette: {
    description: "Switch the palette (gameboy, pico8, cyberpunk or mono), keeping the selected color if the palette has it.",
    args: { id: s.string() },
    run: ({ tx, current }, { id }) => setPalette(tx, current, id),
  },
  selectColor: {
    description: "Choose the #rrggbb color that drawing uses.",
    args: { color: s.string() },
    run: ({ tx }, { color }) => selectColor(tx, color),
  },
});
