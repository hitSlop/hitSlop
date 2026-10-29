import { defineDocument, s } from "@hitslop/tinys";

export const boardShapes = ["landscape", "square", "portrait"] as const;

export default defineDocument({
  boardShape: s.enum(boardShapes, { default: "landscape", description: "The board preset; strokes outside it stay hidden, not removed." }),
  strokes: s.list(s.object({
    geometry: s.string({ description: "The finished stroke outline as an SVG path in board units." }),
    color: s.string({ description: "The ink color, resolved to a hex value when drawn." }),
  })),
}, { initial: { strokes: [] } });
