import { defineDocument, s } from "hitslop";

// Deliberate integration data, independent of any shipped template or its workflow.
export default defineDocument({
  title: s.text({ description: "Fixture text field." }),
  tasks: s.list(s.object({
    text: s.text(),
    done: s.boolean({ default: false }),
    archived: s.boolean({ default: false }),
  })),
});
