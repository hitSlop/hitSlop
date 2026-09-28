import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  focusMinutes: 25,
  restMinutes: 5,
  history: [],
} satisfies Input<typeof schema.fields.node>;
