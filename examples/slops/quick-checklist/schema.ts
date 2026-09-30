import { defineDocument, s, type Value } from "@hitslop/document";
export const checklist = defineDocument({
  title: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean(), archived: s.boolean() })),
});
export type Checklist = Value<typeof checklist.descriptor>;
export default checklist;
