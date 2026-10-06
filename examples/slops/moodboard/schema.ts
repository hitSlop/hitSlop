import { defineDocument, s, type Value } from "@hitslop/document";

export const maxTiles = 24;

const schema = defineDocument({
  title: s.text(),
  // Each tile points at an image the host stores; the bytes are never in the document.
  tiles: s.list(s.object({
    image: s.object({ id: s.string(), mimeType: s.string() }),
    caption: s.string({ maxLength: 60 }),
  })),
});

export type Board = Value<typeof schema.descriptor>;
export type Tile = Board["tiles"][number];
export default schema;
