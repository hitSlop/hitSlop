import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  name: s.text(),
  headline: s.text(),
  location: s.text(),
  bio: s.text(),
  email: s.string(),
  phone: s.string(),
  website: s.string(),
  avatar: s.optional(s.object({ id: s.string(), mimeType: s.string() })),
});

export type ContactCard = Value<typeof schema.fields.node>;
export default schema;
