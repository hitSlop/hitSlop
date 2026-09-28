import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  bpm: 120,
  signature: "4/4",
  volume: 0.75,
  muted: false,
  presets: [60, 90, 120, 140],
} satisfies Input<typeof schema.fields.node>;
