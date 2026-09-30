import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  selectedChannelId: s.string(),
  favoriteChannelIds: s.list(s.string()),
  volume: s.number({ min: 0, max: 1 }),
  muted: s.boolean(),
});

export type AlienRadio = Value<typeof schema.fields.node>;
export default schema;
