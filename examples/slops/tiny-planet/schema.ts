import { defineDocument, s } from "@hitslop/document";

export const kinds = ["tree", "house", "rock", "crystal"] as const;
export type Kind = (typeof kinds)[number];
export const seasons = ["spring", "summer", "autumn", "winter"] as const;
export type Season = (typeof seasons)[number];
export const maxProps = 80;

export default defineDocument({
  name: s.text(),
  seed: s.integer({ min: 0, max: 999999 }),
  season: s.enum(seasons),
  // Where each thing stands, as latitude and longitude in radians. The planet itself is just the seed.
  props: s.list(s.object({ kind: s.enum(kinds), lat: s.number({ min: -1.5708, max: 1.5708 }), lon: s.number({ min: -3.1416, max: 3.1416 }) })),
});
