import { defineDocument, s } from "@hitslop/tinys";

export const stickerKinds = [
  "star", "heart", "smiley", "bolt", "flame", "peace", "skull", "cat",
  "note", "rainbow", "pizza", "alien", "cassette", "hello", "cloud", "fan",
] as const;
export const repeatModes = ["off", "all", "one"] as const;

export default defineDocument({
  ownerName: s.text({ description: "The owner's name, shown as “Name’s Pod”." }),
  videos: s.list(s.object({
    youtubeId: s.string({ maxLength: 11, description: "The 11-character YouTube video id." }),
    title: s.string({ maxLength: 200 }),
    author: s.string({ maxLength: 120, default: "" }),
  })),
  nowPlayingId: s.optional(s.string({ description: "The $id of the current video." })),
  volume: s.number({ min: 0, max: 1, default: 0.7 }),
  repeat: s.enum(repeatModes, { default: "off" }),
  shuffle: s.boolean({ default: false }),
  clicker: s.boolean({ default: true, description: "Play a tick when the wheel turns." }),
  stickers: s.list(s.object({
    kind: s.enum(stickerKinds),
    x: s.number({ min: 0, max: 1, description: "Horizontal position as a fraction of the case width." }),
    y: s.number({ min: 0, max: 1, description: "Vertical position as a fraction of the case height." }),
    rotation: s.number({ min: -180, max: 180, default: 0 }),
    scale: s.number({ min: 0.5, max: 2, default: 1 }),
  })),
}, { initial: {
  ownerName: "",
  videos: [
    { youtubeId: "dQw4w9WgXcQ", title: "Rick Astley - Never Gonna Give You Up (Official Video) (4K Remaster)", author: "Rick Astley" },
    { youtubeId: "fJ9rUzIMcZQ", title: "Queen – Bohemian Rhapsody (Official Video Remastered)", author: "Queen Official" },
    { youtubeId: "5NV6Rdv1a3I", title: "Daft Punk - Get Lucky (Official Audio) ft. Pharrell Williams, Nile Rodgers", author: "DaftPunkVEVO" },
    { youtubeId: "y6120QOlsfU", title: "Darude - Sandstorm", author: "Darude" },
  ],
  stickers: [
    { kind: "star", x: 0.14, y: 0.488, rotation: -14 },
    { kind: "smiley", x: 0.86, y: 0.49, rotation: 10 },
    { kind: "cassette", x: 0.085, y: 0.72, rotation: -8 },
    { kind: "flame", x: 0.915, y: 0.8, rotation: 12 },
  ],
} });
