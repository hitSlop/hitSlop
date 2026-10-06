import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  month: "October",
  currency: "USD",
  sources: [
    { name: "Paycheck", amount: 3200 },
    { name: "Side gig", amount: 600 },
  ],
  buckets: [
    { name: "Rent", amount: 1400, tone: "sky" },
    { name: "Groceries", amount: 480, tone: "leaf" },
    { name: "Bills", amount: 420, tone: "slate" },
    { name: "Fun", amount: 320, tone: "rose" },
    { name: "Savings", amount: 600, tone: "violet" },
  ],
} satisfies Input<typeof schema.descriptor>;
