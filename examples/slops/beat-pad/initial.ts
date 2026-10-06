import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  bpm: 92,
  swing: 18,
  kit: "lofi",
  kick: "x.....x...x.x...",
  snare: "....x.......x...",
  hat: "x.x.x.x.x.x.x.xx",
  bass: "1.....3...2.4...",
} satisfies Input<typeof schema.descriptor>;
