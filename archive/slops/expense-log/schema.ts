import { defineDocument, s, type Value } from "@hitslop/document";

export const categories = ["food", "transit", "coffee", "gear", "bills", "other"] as const;

const schema = defineDocument({
  storeName: s.text(),
  terminalId: s.text(),
  currency: s.string({ maxLength: 3 }),
  items: s.list(s.object({
    title: s.text(),
    amount: s.number(),
    category: s.enum(categories),
    date: s.string(),
    time: s.string(),
  })),
});

export type ExpenseLog = Value<typeof schema.fields.node>;
export type ExpenseItem = ExpenseLog["items"][number];
export default schema;
