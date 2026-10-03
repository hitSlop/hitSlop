import { defineDocument, s, type Value } from "@hitslop/document";
import { STEPS, kits } from "./pattern";

const steps = s.string({ maxLength: STEPS });

const schema = defineDocument({
  bpm: s.integer({ min: 60, max: 200 }),
  swing: s.integer({ min: 0, max: 60 }),
  kit: s.enum(kits),
  kick: steps,
  snare: steps,
  hat: steps,
  bass: steps,
});

export type Beat = Value<typeof schema.descriptor>;
export default schema;
