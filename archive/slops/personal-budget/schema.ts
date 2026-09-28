import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  month: s.string(),
  income: s.number({ min: 0 }),
  savings: s.number({ min: 0 }),
  categories: s.list(s.object({
    name: s.text(),
    allocated: s.number({ min: 0 }),
    spent: s.number({ min: 0 }),
  })),
});

export type PersonalBudget = Value<typeof schema.fields.node>;
export type BudgetCategory = PersonalBudget["categories"][number];
export default schema;
