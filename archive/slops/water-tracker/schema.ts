import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  target: s.number(),
  current: s.number(),
  unit: s.string(),
  logs: s.list(s.object({
    time: s.string(),
    amount: s.number(),
    label: s.string(),
  })),
});

export type WaterTracker = Value<typeof schema.fields.node>;
export default schema;
