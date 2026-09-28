import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  seed: 2718,
  koi: [
    { name: "Mochi", pattern: "kohaku", size: 1.15 },
    { name: "Pip", pattern: "ogon", size: 0.85 },
    { name: "Bean", pattern: "tancho", size: 1 },
  ],
  feedings: 0,
} satisfies Input<typeof schema.fields.node>;
