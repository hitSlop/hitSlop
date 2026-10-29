import { defineDocument, s } from "@hitslop/tinys";
export default defineDocument({ count: s.counter() }, { initial: { count: 0 } });
