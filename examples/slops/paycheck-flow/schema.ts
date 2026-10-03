import { defineDocument, s, type Value } from "@hitslop/document";
import { currencies, tones } from "./money";

const schema = defineDocument({
  month: s.string({ maxLength: 24 }),
  currency: s.enum(currencies),
  sources: s.list(s.object({ name: s.string({ maxLength: 24 }), amount: s.number({ min: 0 }) })),
  buckets: s.list(s.object({ name: s.string({ maxLength: 24 }), amount: s.number({ min: 0 }), tone: s.enum(tones) })),
});

export type Plan = Value<typeof schema.descriptor>;
export default schema;
