import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  pixels: s.list(s.string()),
  paletteId: s.string(),
  selectedColor: s.string(),
});

export type PixelArt = Value<typeof schema.fields.node>;
export default schema;
