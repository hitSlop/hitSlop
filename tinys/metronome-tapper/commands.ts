import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { signatures } from "./schema";
import { removePreset, savePreset, setMuted, setSignature, setTempo, setVolume } from "./metronome";

export default defineCommands(schema, {
  setTempo: {
    description: "Set the tempo in beats per minute (40 to 240).",
    args: { bpm: s.number() },
    run: ({ tx }, { bpm }) => setTempo(tx, bpm),
  },
  setSignature: {
    description: "Set the time signature.",
    args: { signature: s.enum(signatures) },
    run: ({ tx }, { signature }) => setSignature(tx, signature),
  },
  setVolume: {
    description: "Set the click volume (0 to 1).",
    args: { volume: s.number() },
    run: ({ tx }, { volume }) => setVolume(tx, volume),
  },
  setMuted: {
    description: "Mute or unmute the click.",
    args: { muted: s.boolean() },
    run: ({ tx }, { muted }) => setMuted(tx, muted),
  },
  savePreset: {
    description: "Save the current tempo as a preset; presets stay sorted, at most six.",
    args: {},
    run: ({ tx, current }) => savePreset(tx, current),
  },
  removePreset: {
    description: "Remove a saved tempo preset, always keeping at least one.",
    args: { bpm: s.number({ description: "The preset tempo to remove." }) },
    run: ({ tx, current }, { bpm }) => removePreset(tx, current, bpm),
  },
});
