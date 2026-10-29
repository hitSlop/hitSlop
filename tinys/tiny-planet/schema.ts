import { defineDocument, s } from "@hitslop/tinys";

export const kinds = ["tree", "house", "rock", "crystal"] as const;
export type Kind = (typeof kinds)[number];
export const seasons = ["spring", "summer", "autumn", "winter"] as const;
export type Season = (typeof seasons)[number];
export const maxProps = 80;
export const maxSeed = 999999;

export default defineDocument({
  name: s.text({ description: "The planet's name, shown above it." }),
  seed: s.integer({ min: 0, max: maxSeed, description: "The terrain is a pure function of this seed." }),
  season: s.enum(seasons),
  // Where each thing stands, as latitude and longitude in radians. The planet itself is just the seed.
  props: s.list(s.object({
    kind: s.enum(kinds),
    lat: s.number({ min: -1.5708, max: 1.5708 }),
    lon: s.number({ min: -3.1416, max: 3.1416 }),
  })),
}, { initial: {
  name: "Pebble",
  seed: 4242,
  season: "spring",
  props: [
    { kind: "tree", lat: -0.6, lon: 0 },
    { kind: "house", lat: -0.4, lon: 0 },
    { kind: "tree", lat: -0.8, lon: -0.3 },
    { kind: "tree", lat: -0.6, lon: -0.3 },
    { kind: "rock", lat: -0.8, lon: 0.3 },
    { kind: "crystal", lat: -1, lon: 0.3 },
    { kind: "tree", lat: -1, lon: 0 },
  ],
} });
