import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { BPM, STEPS, SWING, clearPattern, kits, setKit, setStep, setSwing, setTempo, setTrack, shufflePattern, tracks } from "./pattern";

const track = s.enum(tracks, { description: "The track: kick, snare, hat or bass." });

export default defineCommands(doc, {
  setStep: {
    description: "Set one step of a track; drums take \".\" or \"x\", bass takes \".\" or a note 1 to 5.",
    args: {
      track,
      step: s.integer({ min: 1, max: STEPS, description: "The step number, 1 to 16." }),
      value: s.string({ description: "\".\" for off, \"x\" for a drum hit, or 1 to 5 for a bass note." }),
    },
    run: ({ tx, current }, args) => setStep(tx, current, args.track, args.step - 1, args.value),
  },
  setTrack: {
    description: "Replace a whole track with a 16-character step string such as \"x...x...x...x...\".",
    args: { track, steps: s.string({ description: "Sixteen characters from the track's alphabet." }) },
    run: ({ tx }, args) => setTrack(tx, args.track, args.steps),
  },
  setTempo: {
    description: "Set the tempo in BPM and, optionally, the swing percentage.",
    args: {
      bpm: s.integer({ min: BPM.min, max: BPM.max, description: "Beats per minute, 60 to 200." }),
      swing: s.optional(s.integer({ min: SWING.min, max: SWING.max, description: "Swing in percent, 0 to 60." })),
    },
    run: ({ tx }, { bpm, swing }) => {
      setTempo(tx, bpm);
      if (swing !== undefined) setSwing(tx, swing);
    },
  },
  setKit: {
    description: "Switch the sound kit.",
    args: { kit: s.enum(kits) },
    run: ({ tx }, { kit }) => setKit(tx, kit),
  },
  shuffle: {
    description: "Replace every track with a fresh random groove.",
    args: {},
    run: (ctx) => shufflePattern(ctx.tx, () => ctx.random()),
  },
  clear: {
    description: "Turn every step of every track off.",
    args: {},
    run: ({ tx }) => clearPattern(tx),
  },
});
