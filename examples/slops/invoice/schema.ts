import { defineDocument, s } from "@hitslop/document";

export const statuses = ["draft", "sent", "paid"] as const;
export const currencies = ["USD", "CAD", "EUR", "GBP", "AUD", "JPY"] as const;

const party = s.object({ name: s.text(), detail: s.text() });

export default defineDocument({
  number: s.text(),
  status: s.enum(statuses),
  issued: s.string(),
  due: s.string(),
  from: party,
  billTo: party,
  items: s.list(s.object({
    description: s.text(),
    quantity: s.number({ min: 0, max: 1_000_000 }),
    rate: s.number({ min: 0, max: 1_000_000_000 }),
  })),
  taxPercent: s.number({ min: 0, max: 100 }),
  notes: s.text(),
  currency: s.enum(currencies),
});
