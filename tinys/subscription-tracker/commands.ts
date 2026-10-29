import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { cadences, categories, currencies } from "./schema";
import { addSubscription, removeSubscription, setActive, setCurrency, updateSubscription } from "./items";

const details = {
  name: s.string({ description: "The service name." }),
  amount: s.number({ description: "The amount per billing cycle, above zero; stored to the cent." }),
  cadence: s.enum(cadences, { description: "monthly or annual." }),
  nextRenewal: s.string({ description: "The next renewal date as YYYY-MM-DD." }),
  category: s.enum(categories, { description: "The service's category." }),
  note: s.string({ description: "A plan or detail; blank for none." }),
};

export default defineCommands(schema, {
  addSubscription: {
    description: "Track a recurring cost and return its stable row ID.",
    args: details,
    run: ({ tx }, args) => addSubscription(tx, args),
  },
  updateSubscription: {
    description: "Change a tracked service's details by its $id, under the same rules as adding one.",
    args: { subscription: s.string({ description: "The subscription's $id." }), ...details },
    run: ({ tx }, { subscription, ...input }) => updateSubscription(tx, subscription, input),
  },
  setActive: {
    description: "Pause or resume tracking a service by its $id; billing is not changed.",
    args: {
      subscription: s.string({ description: "The subscription's $id." }),
      active: s.boolean({ description: "False pauses tracking; true resumes it." }),
    },
    run: ({ tx }, { subscription, active }) => setActive(tx, subscription, active),
  },
  removeSubscription: {
    description: "Delete a tracked service by its $id.",
    args: { subscription: s.string({ description: "The subscription's $id." }) },
    run: ({ tx }, { subscription }) => removeSubscription(tx, subscription),
  },
  setCurrency: {
    description: "Set the document's currency label; amounts are not converted.",
    args: { currency: s.enum(currencies, { description: "The currency code." }) },
    run: ({ tx }, { currency }) => setCurrency(tx, currency),
  },
});
