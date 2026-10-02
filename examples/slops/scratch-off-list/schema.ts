import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  title: s.text(),
  items: s.list(s.object({
    text: s.text(),
    revealed: s.boolean(),
    done: s.boolean(),
  })),
});

export type Card = Value<typeof schema.descriptor>;
export type Item = Card["items"][number];
export default schema;
