import { defineDocument, s } from "hitslop";

export const currencies = ["USD", "CAD", "EUR", "GBP", "AUD", "JPY"] as const;
export const cadences = ["monthly", "annual"] as const;
export const categories = ["Entertainment", "Work", "Home", "Health", "Other"] as const;

export default defineDocument({
  currency: s.enum(currencies),
  subscriptions: s.list(s.object({
    name: s.text(),
    amount: s.number(),
    cadence: s.enum(cadences),
    nextRenewal: s.string(),
    category: s.enum(categories),
    note: s.text(),
    active: s.boolean({ default: true }),
  })),
}, { initial: {
  currency: "USD",
  subscriptions: [
    { name: "Netflix", amount: 15.49, cadence: "monthly", nextRenewal: "2026-09-28", category: "Entertainment", note: "Standard" },
    { name: "iCloud+", amount: 2.99, cadence: "monthly", nextRenewal: "2026-10-07", category: "Home", note: "200 GB" },
    { name: "Adobe Creative Cloud", amount: 54.99, cadence: "monthly", nextRenewal: "2026-09-22", category: "Work", note: "Photography" },
    { name: "Planet Fitness", amount: 24.99, cadence: "monthly", nextRenewal: "2026-09-13", category: "Health", note: "" },
    { name: "Namecheap domain", amount: 13.98, cadence: "annual", nextRenewal: "2026-12-09", category: "Work", note: "hitslop.com" },
    { name: "New York Times", amount: 17, cadence: "monthly", nextRenewal: "2026-10-26", category: "Entertainment", note: "Paused for summer", active: false },
  ],
} });
