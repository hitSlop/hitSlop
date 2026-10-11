import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { CHANNEL_IDS } from "./schema";
import { applyPreset, PRESET_NAMES, setLevel, setMaster, setMuted, setSoloed } from "./mixer";

const channel = s.enum(CHANNEL_IDS, { description: "The channel: rain, thunder, wind, birds or night." });

export default defineCommands(doc, {
  applyPreset: {
    description: "Move every fader to a named preset and clear mute and solo.",
    args: { preset: s.enum(PRESET_NAMES) },
    run: ({ tx }, { preset }) => applyPreset(tx, preset),
  },
  setLevel: {
    description: "Set one channel's fader level from 0 to 100; the preset becomes Custom when the mix no longer matches it.",
    args: { channel, level: s.number({ description: "Fader level, 0 to 100." }) },
    run: ({ tx, current }, { channel, level }) => setLevel(tx, current, channel, level),
  },
  setMaster: {
    description: "Set the master volume from 0 to 100.",
    args: { level: s.number({ description: "Master volume, 0 to 100." }) },
    run: ({ tx }, { level }) => setMaster(tx, level),
  },
  setMute: {
    description: "Mute or unmute one channel.",
    args: { channel, muted: s.boolean() },
    run: ({ tx }, { channel, muted }) => setMuted(tx, channel, muted),
  },
  setSolo: {
    description: "Solo or unsolo one channel; while any channel is soloed only soloed channels are heard.",
    args: { channel, soloed: s.boolean() },
    run: ({ tx }, { channel, soloed }) => setSoloed(tx, channel, soloed),
  },
});
