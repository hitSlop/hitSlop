import { defineDocument, s, type Value } from "@hitslop/document";
/** What the glass counts down to. `start` and `end` are epoch milliseconds: when it was
 * turned over and when its sand runs out; 0 until the first turn. */
export const hourglass = defineDocument({
  title: s.text(),
  start: s.number({ min: 0 }),
  end: s.number({ min: 0 }),
});
export type Hourglass = Value<typeof hourglass.descriptor>;
export default hourglass;
