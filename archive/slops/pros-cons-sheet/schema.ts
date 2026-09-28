import { defineDocument, s, type Value } from "@hitslop/document";

export const sides = ["pro", "con"] as const;

const schema = defineDocument({
  question: s.text(),
  date: s.text(),
  status: s.enum(["evaluating", "leaning_pro", "leaning_con", "decided_pro", "decided_con"]),
  factors: s.list(s.object({
    text: s.text(),
    weight: s.integer({ min: 1, max: 5 }),
    side: s.enum(sides),
  })),
  verdict: s.text(),
});

export type Decision = Value<typeof schema.fields.node>;
export type Factor = Decision["factors"][number];
export type Side = Factor["side"];
export default schema;
