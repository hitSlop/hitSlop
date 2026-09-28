import { defineDocument, s, type Value } from "@hitslop/document";

export const currencies = ["USD", "CAD", "EUR", "GBP", "AUD", "JPY"] as const;
export const cadences = ["monthly", "annual"] as const;
export const categories = ["Entertainment", "Work", "Home", "Health", "Other"] as const;

const schema = defineDocument({
  currency: s.enum(currencies),
  subscriptions: s.list(s.object({
    name: s.text(),
    amount: s.number(),
    cadence: s.enum(cadences),
    nextRenewal: s.string(),
    category: s.enum(categories),
    note: s.text(),
    active: s.boolean(),
  })),
});

export type Tracker = Value<typeof schema.fields.node>;
export type Subscription = Tracker["subscriptions"][number];
export type Currency = (typeof currencies)[number];
export type Cadence = (typeof cadences)[number];
export type Category = (typeof categories)[number];
export default schema;
