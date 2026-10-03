import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Moodboard",
  tiles: [],
} satisfies Input<typeof schema.descriptor>;
