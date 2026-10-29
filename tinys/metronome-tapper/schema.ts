import { defineDocument, s } from "@hitslop/tinys";

export const signatures = ["2/4", "3/4", "4/4", "6/8"] as const;

export default defineDocument({
  bpm: s.integer({ min: 40, max: 240 }),
  signature: s.enum(signatures),
  volume: s.number({ min: 0, max: 1 }),
  muted: s.boolean(),
  presets: s.list(s.integer({ min: 40, max: 240 })),
}, {
  initial: {
    bpm: 120,
    signature: "4/4",
    volume: 0.75,
    muted: false,
    presets: [60, 90, 120, 140],
  },
});
