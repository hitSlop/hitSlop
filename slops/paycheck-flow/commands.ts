import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { currencies, tones } from "./money";
import { addIncome, addSpending, removeRow, setCurrency, setMonth, updateRow } from "./plan";

const amountMinor = (description: string) =>
  s.integer({ min: 0, description: `${description} In minor units: cents, or whole yen for JPY.` });
const row = s.string({ description: "The income or spending row's $id." });

export default defineCommands(doc, {
  setMonth: {
    description: "Set the month named at the top of the plan (up to 24 characters).",
    args: { month: s.string() },
    run: ({ tx }, { month }) => setMonth(tx, month),
  },
  setCurrency: {
    description: "Switch the plan's currency, rescaling stored amounts so the numbers shown stay the same.",
    args: { currency: s.enum(currencies) },
    run: ({ tx, current }, { currency }) => setCurrency(tx, current, currency),
  },
  addIncome: {
    description: "Add a money-in row (at most 4) and return its stable row ID.",
    args: { name: s.string(), amountMinor: s.optional(amountMinor("The monthly income.")) },
    run: ({ tx, current }, { name, amountMinor }) => addIncome(tx, current, name, amountMinor),
  },
  addSpending: {
    description: "Add a money-out row (at most 7) and return its stable row ID; the colour defaults to the next in turn.",
    args: {
      name: s.string(),
      amountMinor: s.optional(amountMinor("The monthly spending.")),
      tone: s.optional(s.enum(tones)),
    },
    run: ({ tx, current }, { name, amountMinor, tone }) => addSpending(tx, current, name, amountMinor, tone),
  },
  updateRow: {
    description: "Change the name, amount or colour of an income or spending row; income rows take no colour.",
    args: {
      row,
      name: s.optional(s.string()),
      amountMinor: s.optional(amountMinor("The new amount.")),
      tone: s.optional(s.enum(tones)),
    },
    run: ({ tx, current }, { row, ...change }) => updateRow(tx, current, row, change),
  },
  removeRow: {
    description: "Remove an income or spending row from the plan.",
    args: { row },
    run: ({ tx, current }, { row }) => removeRow(tx, current, row),
  },
});
