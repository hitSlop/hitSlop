import { defineDocument, s, type Value } from "@hitslop/document";

export const kinds = ["face", "cheek", "eyes", "lips", "skin", "hair", "body"] as const;

const schema = defineDocument({
  title: s.text(),
  products: s.list(s.object({
    name: s.text(),
    brand: s.text(),
    kind: s.enum(kinds),
    shade: s.string({ maxLength: 7 }),
    left: s.integer({ min: 0, max: 100 }),
    opened: s.optional(s.string({ maxLength: 10 })),
    pao: s.integer({ min: 1, max: 60 }),
  })),
});

export type Stash = Value<typeof schema.descriptor>;
export type Product = Stash["products"][number];
export default schema;
