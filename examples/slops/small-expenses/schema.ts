import { defineDocument, s } from "@hitslop/document";
export const expenses=defineDocument({
  title:s.text(),
  currency:s.enum(["CAD","USD","EUR"]),
  items:s.list(s.object({merchant:s.text(),amountMinor:s.integer({min:0}),note:s.optional(s.string()),settled:s.boolean()})),
});
export const fields=expenses.fields;
