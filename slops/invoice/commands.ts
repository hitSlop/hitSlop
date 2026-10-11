import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { currencies, statuses } from "./schema";
import { addItem, removeItem, setCurrency, setStatus, setTaxPercent } from "./items";

export default defineCommands(doc, {
  addLine: {
    description: "Add an empty line item and return its stable row ID.",
    args: {},
    run: ({ tx }) => addItem(tx),
  },
  removeLine: {
    description: "Remove a line item from the invoice.",
    args: { item: s.string({ description: "The line item's $id." }) },
    run: ({ tx }, { item }) => removeItem(tx, item),
  },
  setStatus: {
    description: "Set the invoice status to draft, sent or paid.",
    args: { status: s.enum(statuses) },
    run: ({ tx }, { status }) => setStatus(tx, status),
  },
  setCurrency: {
    description: "Set the currency the invoice amounts are shown in.",
    args: { currency: s.enum(currencies) },
    run: ({ tx }, { currency }) => setCurrency(tx, currency),
  },
  setTaxPercent: {
    description: "Set the tax rate as a percentage from 0 to 100.",
    args: { percent: s.number({ min: 0, max: 100 }) },
    run: ({ tx }, { percent }) => setTaxPercent(tx, percent),
  },
});
