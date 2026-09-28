import { defineDocument, s } from "@hitslop/document";

export const boardShapes = ["landscape", "square", "portrait"] as const;
export default defineDocument({
  boardShape: s.enum(boardShapes),
  strokes: s.list(s.object({ geometry: s.string(), color: s.string() })),
});
