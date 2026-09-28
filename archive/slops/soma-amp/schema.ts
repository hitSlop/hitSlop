import { defineDocument, s, type Value } from "@hitslop/document";

export const bands = ["preamp", "hz60", "hz170", "hz310", "hz600", "hz1000", "hz3000", "hz6000", "hz12000", "hz14000", "hz16000"] as const;
const schema = defineDocument({
  selectedStationId: s.string(),
  volume: s.number(),
  balance: s.number(),
  milkdropOpen: s.boolean(),
  equalizer: s.object({
    on: s.boolean(),
    preamp: s.number(), hz60: s.number(), hz170: s.number(), hz310: s.number(),
    hz600: s.number(), hz1000: s.number(), hz3000: s.number(), hz6000: s.number(),
    hz12000: s.number(), hz14000: s.number(), hz16000: s.number(),
  }),
  skin: s.object({ id: s.optional(s.string()), name: s.optional(s.string()) }),
});
export type SomaAmp = Value<typeof schema.fields.node>;
export default schema;
