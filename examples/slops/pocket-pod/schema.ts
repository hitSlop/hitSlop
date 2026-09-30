import { defineDocument, s, type Value } from "@hitslop/document";

export const stickerKinds = [
  "star", "heart", "smiley", "bolt", "flame", "peace", "skull", "cat",
  "note", "rainbow", "pizza", "alien", "cassette", "hello", "cloud", "fan",
] as const;
export type StickerKind = (typeof stickerKinds)[number];
export const repeatModes = ["off", "all", "one"] as const;
export type RepeatMode = (typeof repeatModes)[number];

const schema = defineDocument({
  ownerName: s.text(),
  videos: s.list(s.object({
    youtubeId: s.string({ maxLength: 11 }),
    title: s.string({ maxLength: 200 }),
    author: s.string({ maxLength: 120 }),
  })),
  nowPlayingId: s.optional(s.string()),
  volume: s.number({ min: 0, max: 1 }),
  repeat: s.enum(repeatModes),
  shuffle: s.boolean(),
  clicker: s.boolean(),
  stickers: s.list(s.object({
    kind: s.enum(stickerKinds),
    x: s.number({ min: 0, max: 1 }),
    y: s.number({ min: 0, max: 1 }),
    rotation: s.number({ min: -180, max: 180 }),
    scale: s.number({ min: 0.5, max: 2 }),
  })),
});

export type Pod = Value<typeof schema.descriptor>;
export type Video = Pod["videos"][number];
export type Sticker = Pod["stickers"][number];
export default schema;
