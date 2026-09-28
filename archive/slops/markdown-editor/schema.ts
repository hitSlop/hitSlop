import { defineDocument, s, type Value } from "@hitslop/document";

export const modes = ["inplace", "split", "preview"] as const;
export const themes = ["paper", "dark"] as const;

const schema = defineDocument({
  title: s.text(),
  content: s.text(),
  mode: s.enum(modes),
  theme: s.enum(themes),
});

export type MarkdownDoc = Value<typeof schema.fields.node>;
export type Mode = MarkdownDoc["mode"];
export default schema;
