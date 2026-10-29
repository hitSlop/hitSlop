import { defineDocument, s } from "@hitslop/tinys";

export const CHANNEL_IDS = ["rain", "thunder", "wind", "birds", "night"] as const;
export const presets = ["Lo-Fi Rain", "Forest Dawn", "Midnight Storm", "Cozy Evening", "Custom"] as const;

const level = s.number({ min: 0, max: 100, description: "Fader level, 0 to 100." });
const muted = s.boolean({ default: false });
const soloed = s.boolean({ default: false });

export default defineDocument({
  preset: s.enum(presets, { description: "The preset the faders match, or Custom once a fader moves." }),
  master: s.number({ min: 0, max: 100, description: "Master volume, 0 to 100." }),
  channels: s.object({ rain: level, thunder: level, wind: level, birds: level, night: level }),
  muted: s.object({ rain: muted, thunder: muted, wind: muted, birds: muted, night: muted }),
  soloed: s.object({ rain: soloed, thunder: soloed, wind: soloed, birds: soloed, night: soloed }),
}, { initial: {
  preset: "Lo-Fi Rain",
  master: 80,
  channels: { rain: 75, thunder: 20, wind: 35, birds: 0, night: 40 },
  muted: {},
  soloed: {},
} });
