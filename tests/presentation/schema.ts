import { defineDocument, s } from "@hitslop/pop";
export default defineDocument({ count: s.counter() }, { initial: { count: 0 } });
