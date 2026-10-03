import { defineDocument, s, type Value } from "@hitslop/document";

export const shapeKinds = ["circle", "square"] as const;
export const tones = ["tomato", "cobalt", "mustard"] as const;
export const maxShapes = 5;

const schema = defineDocument({
  title: s.text(),
  body: s.text(),
  // Centre and size as fractions of the page width (y: of its height), so a poster looks the same at any window size.
  shapes: s.list(s.object({
    kind: s.enum(shapeKinds),
    tone: s.enum(tones),
    x: s.number({ min: 0, max: 1 }),
    y: s.number({ min: 0, max: 1 }),
    size: s.number({ min: 0.12, max: 0.6 }),
  })),
});

export type Poster = Value<typeof schema.descriptor>;
export type Shape = Poster["shapes"][number];
export default schema;
