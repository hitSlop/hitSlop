import { defineDocument, s } from "hitslop";
export default defineDocument({ first: s.text(), second: s.text() }, { initial: { first: "First field", second: "A second field with words" } });
