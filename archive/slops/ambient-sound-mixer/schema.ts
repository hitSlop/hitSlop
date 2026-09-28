import { defineDocument, s, type Value } from "@hitslop/document";

export const CHANNEL_IDS = ["rain", "thunder", "wind", "birds", "night"] as const;
export type ChannelID = (typeof CHANNEL_IDS)[number];
export type ChannelLevels = Record<ChannelID, number>;

export const presets = ["Lo-Fi Rain", "Forest Dawn", "Midnight Storm", "Cozy Evening", "Custom"] as const;

const level = s.number({ min: 0, max: 100 });
const flag = s.boolean();

const schema = defineDocument({
  preset: s.enum(presets),
  master: s.number({ min: 0, max: 100 }),
  playing: s.boolean(),
  channels: s.object({
    rain: level,
    thunder: level,
    wind: level,
    birds: level,
    night: level,
  }),
  muted: s.object({
    rain: flag,
    thunder: flag,
    wind: flag,
    birds: flag,
    night: flag,
  }),
  soloed: s.object({
    rain: flag,
    thunder: flag,
    wind: flag,
    birds: flag,
    night: flag,
  }),
});

export type Mixer = Value<typeof schema.fields.node>;
export default schema;
