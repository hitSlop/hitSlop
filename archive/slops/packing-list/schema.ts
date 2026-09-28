import { defineDocument, s, type Value } from "@hitslop/document";

export const stampColors = ["cobalt", "vermilion", "emerald", "amber", "plum", "slate"] as const;

const schema = defineDocument({
  tripTitle: s.text(),
  destination: s.text(),
  departureDate: s.string(),
  traveler: s.text(),
  bagTag: s.string(),
  flag: s.string(),
  categories: s.list(s.object({
    key: s.string(),
    name: s.string(),
    tagCode: s.string(),
    color: s.enum(stampColors),
  })),
  items: s.list(s.object({
    key: s.string(),
    text: s.text(),
    quantity: s.integer({ min: 1, max: 9 }),
    packed: s.boolean(),
    essential: s.boolean(),
  })),
});

export type PackingList = Value<typeof schema.fields.node>;
export type PackingItem = PackingList["items"][number];
export type PackingCategory = PackingList["categories"][number];
export default schema;
