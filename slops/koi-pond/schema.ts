import { defineDocument, s } from "hitslop";

export const koiPatterns = ["kohaku", "showa", "ogon", "asagi", "tancho"] as const;
export const maxKoi = 9;

export default defineDocument({
  seed: s.integer({ min: 0, max: 999999, description: "Seeds the watercolor painting of the pond, stones and lily pads." }),
  koi: s.list(s.object({
    name: s.text(),
    pattern: s.enum(koiPatterns),
    size: s.number({ min: 0.7, max: 1.3, description: "Body length relative to a standard koi." }),
  })),
  feedings: s.counter({ description: "How many times the koi have been fed." }),
}, { initial: {
  seed: 2718,
  koi: [
    { name: "Mochi", pattern: "kohaku", size: 1.15 },
    { name: "Pip", pattern: "ogon", size: 0.85 },
    { name: "Bean", pattern: "tancho", size: 1 },
  ],
  feedings: 0,
} });
