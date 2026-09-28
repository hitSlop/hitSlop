import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  displayName: s.text(),
  package: s.optional(s.object({
    id: s.string(),
    name: s.string(),
    mimeType: s.string(),
    byteLength: s.integer({ min: 0 }),
  })),
});

export type CodexPet = Value<typeof schema.fields.node>;
export default schema;
