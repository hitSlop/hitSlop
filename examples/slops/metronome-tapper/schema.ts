import { defineDocument, s, type Value } from "@hitslop/document";

export const signatures = ["2/4", "3/4", "4/4", "6/8"] as const;

const schema = defineDocument({
  bpm: s.integer({ min: 40, max: 240 }),
  signature: s.enum(signatures),
  volume: s.number({ min: 0, max: 1 }),
  muted: s.boolean(),
  presets: s.list(s.integer({ min: 40, max: 240 })),
});

export type MetronomeState = Value<typeof schema.fields.node>;
export type TimeSignature = MetronomeState["signature"];
export default schema;
