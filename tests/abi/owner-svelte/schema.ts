import { defineDocument, s } from "@hitslop/document";
export default defineDocument({ title: s.text(), done: s.boolean(), hits: s.counter(), rows: s.list(s.object({ text: s.text(), done: s.boolean() })) });
