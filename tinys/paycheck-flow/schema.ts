import { defineDocument, s } from "@hitslop/tinys";
import { currencies, tones } from "./money";

const minor = { min: 0, description: "Amount in the currency's minor unit: cents, or whole yen for JPY." };

export default defineDocument({
  month: s.string({ maxLength: 24 }),
  currency: s.enum(currencies),
  sources: s.list(s.object({
    name: s.string({ maxLength: 24 }),
    amountMinor: s.integer({ ...minor, default: 0 }),
  })),
  buckets: s.list(s.object({
    name: s.string({ maxLength: 24 }),
    amountMinor: s.integer({ ...minor, default: 0 }),
    tone: s.enum(tones),
  })),
}, { initial: {
  month: "October",
  currency: "USD",
  sources: [
    { name: "Paycheck", amountMinor: 320000 },
    { name: "Side gig", amountMinor: 60000 },
  ],
  buckets: [
    { name: "Rent", amountMinor: 140000, tone: "sky" },
    { name: "Groceries", amountMinor: 48000, tone: "leaf" },
    { name: "Bills", amountMinor: 42000, tone: "slate" },
    { name: "Fun", amountMinor: 32000, tone: "rose" },
    { name: "Savings", amountMinor: 60000, tone: "violet" },
  ],
} });
