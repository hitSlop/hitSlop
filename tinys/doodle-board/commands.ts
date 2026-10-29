import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { boardShapes } from "./schema";
import { clearBoard, drawStroke, setBoardShape } from "./drawing";

export default defineCommands(schema, {
  drawStroke: {
    description: "Draw one freehand stroke through the given board points and return its stable row ID.",
    args: {
      points: s.string({ minLength: 1, description: "Space-separated x,y pairs in board units, such as \"120,80 300,220\". Landscape is 1000 × 700, square 1000 × 1000, portrait 700 × 1000." }),
      color: s.string({ description: "Ink as a hex value, such as #343744 (charcoal), #d94839 (coral), #2d63b8 (blue), #238063 (green) or #8852a0 (purple)." }),
      size: s.optional(s.number({ min: 1, max: 64, description: "Line weight from 1 to 64; fine is 4, medium 9, bold 18. Defaults to 9." })),
    },
    run: ({ tx }, { points, color, size }) => drawStroke(tx, points, color, size ?? undefined),
  },
  removeStroke: {
    description: "Erase one whole stroke by its $id.",
    args: { stroke: s.string({ description: "The stroke's $id." }) },
    run: ({ tx }, { stroke }) => tx.fields.strokes.remove(stroke),
  },
  clearBoard: {
    description: "Remove every stroke, including ink outside the current board, and return the number removed.",
    args: {},
    run: ({ tx, current }) => clearBoard(tx, current),
  },
  setBoardShape: {
    description: "Choose the board preset; strokes outside the new board stay hidden, not removed.",
    args: { shape: s.enum(boardShapes, { description: "landscape (1000 × 700), square (1000 × 1000) or portrait (700 × 1000)." }) },
    run: ({ tx }, { shape }) => setBoardShape(tx, shape),
  },
});
