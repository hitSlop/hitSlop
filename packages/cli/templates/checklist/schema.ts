import { defineDocument, s } from "@hitslop/document";
export default defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
