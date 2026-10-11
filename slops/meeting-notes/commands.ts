import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addAction, addAgendaTopic, addAttendee, addDecision, removeAction, setActionDone } from "./meeting";

export default defineCommands(doc, {
  addAttendee: {
    description: "Add a person to the attendee list and return the new attendee count.",
    args: { name: s.string() },
    run: ({ tx, current }, { name }) => {
      addAttendee(tx, name);
      return current.attendees.length + 1;
    },
  },
  addAgendaTopic: {
    description: "Add an agenda topic, unchecked, and return its stable row ID.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => addAgendaTopic(tx, text),
  },
  addDecision: {
    description: "Record a decision and return its stable row ID.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => addDecision(tx, text),
  },
  addAction: {
    description: "Add a follow-up action with an owner, unchecked, and return its stable row ID.",
    args: { text: s.string(), owner: s.string({ description: "Who owns the action; may be empty." }) },
    run: ({ tx }, { text, owner }) => addAction(tx, text, owner),
  },
  setActionDone: {
    description: "Mark a follow-up action done or open again. Returns nothing.",
    args: {
      action: s.string({ description: "The action's $id." }),
      done: s.boolean(),
    },
    run: ({ tx }, { action, done }) => setActionDone(tx, action, done),
  },
  removeAction: {
    description: "Remove a follow-up action. Returns nothing.",
    args: { action: s.string({ description: "The action's $id." }) },
    run: ({ tx }, { action }) => removeAction(tx, action),
  },
});
