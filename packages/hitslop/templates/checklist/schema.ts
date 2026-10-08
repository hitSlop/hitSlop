import { defineDocument, s } from "hitslop";
// Fields a write leaves out take their default: text starts empty, done starts false.
export default defineDocument({
  title: s.text({ description: "The checklist's title." }),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean({ default: false }) }), { description: "Tasks in their display order." }),
});
