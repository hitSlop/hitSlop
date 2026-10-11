import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { shapeKinds, tones } from "./schema";
import { addShape, moveShape, removeShape, resizeShape, setShapeTone, setText } from "./poster";

const shape = s.string({ description: "The shape's $id." });

export default defineCommands(doc, {
  addShape: {
    description: "Add a circle or square to the poster (at most 5) and return its stable row ID.",
    args: {
      kind: s.enum(shapeKinds, { description: "circle or square." }),
      tone: s.optional(s.enum(tones, { description: "tomato, cobalt or mustard; defaults to the next colour in turn." })),
      x: s.optional(s.number({ description: "Centre as a fraction of the page width, 0 to 1; defaults to 0.5." })),
      y: s.optional(s.number({ description: "Centre as a fraction of the page height, 0 to 1; defaults to 0.55." })),
      size: s.optional(s.number({ description: "Diameter or side as a fraction of the page width, 0.12 to 0.6; defaults to 0.28 (circle) or 0.22 (square)." })),
    },
    run: ({ tx, current }, { kind, ...options }) => addShape(tx, current, kind, options),
  },
  moveShape: {
    description: "Move a shape's centre to fractions of the page width and height; the words reflow around it.",
    args: {
      shape,
      x: s.number({ description: "Centre as a fraction of the page width, 0 to 1." }),
      y: s.number({ description: "Centre as a fraction of the page height, 0 to 1." }),
    },
    run: ({ tx }, { shape, x, y }) => moveShape(tx, shape, x, y),
  },
  resizeShape: {
    description: "Set a shape's diameter or side as a fraction of the page width.",
    args: { shape, size: s.number({ description: "From 0.12 to 0.6." }) },
    run: ({ tx }, { shape, size }) => resizeShape(tx, shape, size),
  },
  setShapeTone: {
    description: "Change a shape's colour.",
    args: { shape, tone: s.enum(tones, { description: "tomato, cobalt or mustard." }) },
    run: ({ tx }, { shape, tone }) => setShapeTone(tx, shape, tone),
  },
  removeShape: {
    description: "Remove a shape from the poster.",
    args: { shape },
    run: ({ tx }, { shape }) => removeShape(tx, shape),
  },
  setText: {
    description: "Replace the poster's title, body text, or both.",
    args: {
      title: s.optional(s.string({ description: "The new headline." })),
      body: s.optional(s.string({ description: "The new body text; blank lines separate paragraphs." })),
    },
    run: ({ tx }, text) => setText(tx, text),
  },
});
