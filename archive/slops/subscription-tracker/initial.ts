import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  currency: "USD",
  subscriptions: [
    { name: "Netflix", amount: 15.49, cadence: "monthly", nextRenewal: "2026-09-28", category: "Entertainment", note: "Standard", active: true },
    { name: "iCloud+", amount: 2.99, cadence: "monthly", nextRenewal: "2026-10-07", category: "Home", note: "200 GB", active: true },
    { name: "Adobe Creative Cloud", amount: 54.99, cadence: "monthly", nextRenewal: "2026-09-22", category: "Work", note: "Photography", active: true },
    { name: "Planet Fitness", amount: 24.99, cadence: "monthly", nextRenewal: "2026-09-13", category: "Health", note: "", active: true },
    { name: "Namecheap domain", amount: 13.98, cadence: "annual", nextRenewal: "2026-12-09", category: "Work", note: "hitslop.com", active: true },
    { name: "New York Times", amount: 17, cadence: "monthly", nextRenewal: "2026-10-26", category: "Entertainment", note: "Paused for summer", active: false },
  ],
} satisfies Input<typeof schema.fields.node>;
