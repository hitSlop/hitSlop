import type { Input } from "@hitslop/document";
import schema from "./schema";
export default {
  name: "",
  birthday: "",
  color: "peach",
  lastCaregiverId: "",
  bottleUnit: "mL",
  caregivers: [{ name: "You", archived: false }],
  entries: [],
} satisfies Input<typeof schema.descriptor>;
