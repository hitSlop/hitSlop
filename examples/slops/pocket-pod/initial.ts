import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  ownerName: "",
  videos: [
    { youtubeId: "dQw4w9WgXcQ", title: "Rick Astley - Never Gonna Give You Up (Official Video) (4K Remaster)", author: "Rick Astley" },
    { youtubeId: "fJ9rUzIMcZQ", title: "Queen – Bohemian Rhapsody (Official Video Remastered)", author: "Queen Official" },
    { youtubeId: "5NV6Rdv1a3I", title: "Daft Punk - Get Lucky (Official Audio) ft. Pharrell Williams, Nile Rodgers", author: "DaftPunkVEVO" },
    { youtubeId: "y6120QOlsfU", title: "Darude - Sandstorm", author: "Darude" },
  ],
  volume: 0.7,
  repeat: "off",
  shuffle: false,
  clicker: true,
  stickers: [
    { kind: "star", x: 0.14, y: 0.488, rotation: -14, scale: 1 },
    { kind: "smiley", x: 0.86, y: 0.49, rotation: 10, scale: 1 },
    { kind: "cassette", x: 0.085, y: 0.72, rotation: -8, scale: 1 },
    { kind: "flame", x: 0.915, y: 0.8, rotation: 12, scale: 1 },
  ],
} satisfies Input<typeof schema.descriptor>;
