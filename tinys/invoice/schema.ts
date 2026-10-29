import { defineDocument, s } from "@hitslop/tinys";

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
}, { initial: {
  number: "INV-001",
  status: "draft",
  issued: "2026-09-23",
  due: "2026-10-07",
  from: {
    name: "Northwind Studio",
    detail: "14 Market Lane\nPortland, OR",
  },
  billTo: {
    name: "Harbor Goods",
    detail: "88 Pier Street\nSeattle, WA",
  },
  items: [
    {
      description: "Brand system and invoice template",
      quantity: 1,
      rate: 2400,
    },
  ],
  taxPercent: 0,
  notes: "Due on receipt. Thank you.",
  currency: "USD",
} });
