import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  question: "",
  date: "9/16/2026",
  status: "evaluating",
  factors: [],
  verdict: "",
} satisfies Input<typeof schema.fields.node>;
