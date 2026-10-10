import { defineDocument, s } from "hitslop";
export default defineDocument({
  note: s.text(),
  hits: s.counter(),
  lastTarget: s.string({ maxLength: 24 }),
}, { initial: { note: "Type here, then capture.", hits: 0, lastTarget: "None" } });
