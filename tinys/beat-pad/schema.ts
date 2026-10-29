import { defineDocument, s } from "@hitslop/tinys";
import { BPM, STEPS, SWING, kits } from "./pattern";

const steps = s.string({
  minLength: STEPS,
  maxLength: STEPS,
  description: "Sixteen steps, one character each: drums are \".\" or \"x\"; bass is \".\" or a note 1 to 5.",
});

export default defineDocument({
  bpm: s.integer({ min: BPM.min, max: BPM.max, description: "Tempo in beats per minute." }),
  swing: s.integer({ min: SWING.min, max: SWING.max, description: "Swing on the off sixteenths, in percent." }),
  kit: s.enum(kits),
  kick: steps,
  snare: steps,
  hat: steps,
  bass: steps,
}, { initial: {
  bpm: 92,
  swing: 18,
  kit: "lofi",
  kick: "x.....x...x.x...",
  snare: "....x.......x...",
  hat: "x.x.x.x.x.x.x.xx",
  bass: "1.....3...2.4...",
} });
