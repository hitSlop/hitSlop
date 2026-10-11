import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { detailFields, setField, vCardText } from "./shared";

export default defineCommands(doc, {
  setField: {
    description: "Set one contact detail (name, headline, location, bio, email, phone or website) to the given text.",
    args: {
      field: s.enum(detailFields, { description: "The detail to set." }),
      value: s.string({ description: "The new text; an empty string clears it." }),
    },
    run: ({ tx }, { field, value }) => setField(tx, field, value),
  },
  vCard: {
    description: "Return the card as vCard 3.0 text, the same content Save vCard writes.",
    args: {},
    run: ({ current }) => vCardText(current),
  },
});
