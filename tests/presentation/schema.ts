import { defineDocument, s } from "hitslop";
export default defineDocument({ count: s.counter() }, { initial: { count: 0 } });
