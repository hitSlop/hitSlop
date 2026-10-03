import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
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
} satisfies Input<typeof schema.descriptor>;
