import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  habits: [
    { name: "Morning pages", color: "mint", checkins: {} },
    { name: "Walk outside", color: "coral", checkins: {} },
    { name: "Read 20 min", color: "butter", checkins: {} },
  ],
} satisfies Input<typeof schema.fields.node>;
