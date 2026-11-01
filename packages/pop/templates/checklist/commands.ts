import { defineCommands } from "@hitslop/pop/commands";
import { s } from "@hitslop/pop";
import schema, { addTask } from "./schema";

export default defineCommands(schema, {
  addTask: {
    description: "Add an unfinished task and return its row ID.",
    args: { text: s.string() },
    run: ({ tx }, { text }) => addTask(tx, text),
  },
});
