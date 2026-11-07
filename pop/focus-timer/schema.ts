import { defineDocument, s } from "@hitslop/pop";
export default defineDocument({ minutes: s.integer({ min: 1, max: 120, default: 25 }) });
