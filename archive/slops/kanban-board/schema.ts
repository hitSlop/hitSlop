import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  title: s.text(),
  doneLaneKey: s.optional(s.string()),
  lanes: s.list(s.object({
    laneKey: s.string(),
    title: s.text(),
    limit: s.optional(s.integer({ min: 0, max: 999 })),
  })),
  cards: s.list(s.object({
    laneKey: s.string(),
    title: s.text(),
    note: s.text(),
    tag: s.text(),
    order: s.integer({ min: 0 }),
  })),
});

export type Board = Value<typeof schema.fields.node>;
export type Lane = Board["lanes"][number];
export type Card = Board["cards"][number];
export default schema;
