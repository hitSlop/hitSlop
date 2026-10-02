import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  displayName: "Bubu",
} satisfies Input<typeof schema.descriptor>;
