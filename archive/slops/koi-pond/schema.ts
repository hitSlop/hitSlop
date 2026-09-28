import { defineDocument, s } from "@hitslop/document";

export const koiPatterns = ["kohaku", "showa", "ogon", "asagi", "tancho"] as const;
export type KoiPattern = (typeof koiPatterns)[number];
export const maxKoi = 9;

export default defineDocument({
  seed: s.integer({ min: 0, max: 999999 }),
  koi: s.list(s.object({ name: s.text(), pattern: s.enum(koiPatterns), size: s.number({ min: 0.7, max: 1.3 }) })),
  feedings: s.counter(),
});
