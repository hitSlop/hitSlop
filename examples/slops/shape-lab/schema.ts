import { defineDocument, s } from "@hitslop/document";
export default defineDocument({
  note: s.text(),
  hits: s.counter(),
  lastTarget: s.string({ maxLength: 24 }),
});
