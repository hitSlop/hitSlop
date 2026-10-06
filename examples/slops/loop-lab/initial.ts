import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  bpm: 112,
  tracks: [
    { kind: "drums", name: "Drums", code: "bd [~ bd] sd ~, hh*8", voice: "sine", gain: 0.8, filter: 9000, room: 0.1, muted: false },
    { kind: "synth", name: "Bass", code: "c2 ~ <eb2 f2> ~ g1 ~ c2 ~", voice: "sawtooth", gain: 0.6, filter: 700, room: 0, muted: false },
    { kind: "synth", name: "Melody", code: "e4 g4 <b4 a4> e5(3,8)", voice: "triangle", gain: 0.45, filter: 4000, room: 0.4, muted: false },
  ],
} satisfies Input<typeof schema.descriptor>;
