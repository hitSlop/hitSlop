import { defineDocument, s } from "hitslop";
import { statuses } from "./statuses";

export const sides = ["pro", "con"] as const;

export default defineDocument({
  question: s.text(),
  date: s.text(),
  status: s.enum(statuses, { default: "evaluating" }),
  factors: s.list(s.object({
    text: s.text(),
    weight: s.integer({ min: 1, max: 5 }),
    side: s.enum(sides),
  })),
  verdict: s.text(),
}, { initial: {
  question: "",
  date: "9/16/2026",
  status: "evaluating",
  factors: [],
  verdict: "",
} });
