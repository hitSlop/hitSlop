import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text(),
  currency: s.enum(["CAD", "USD", "EUR"]),
  items: s.list(s.object({
    merchant: s.text(),
    amountMinor: s.integer({ min: 0 }),
    note: s.optional(s.string()),
    settled: s.boolean({ default: false }),
  })),
}, { initial: {
  title: "The little things",
  currency: "CAD",
  items: [
    { merchant: "Neighbourhood coffee", amountMinor: 475, note: "Morning catch-up" },
    { merchant: "Studio supplies", amountMinor: 2400 },
    { merchant: "Train home", amountMinor: 650, settled: true },
  ],
} });
