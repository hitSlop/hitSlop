import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { addTask } from "./schema";

export default defineCommands(doc, {
  addTask: {
    description: "Add an unfinished task and return its row ID.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => addTask(tx, text),
  },
});
