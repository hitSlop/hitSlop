import { defineDocument, s } from "hitslop";
export default defineDocument({
  title: s.text({ description: "The checklist's title." }),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() }), { description: "Tasks in their display order." }),
});
