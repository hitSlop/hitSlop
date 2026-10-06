import { defineDocument, s, type Value } from "hitslop";
/** What the glass counts down to. `start` and `end` are epoch milliseconds: when it was
 * turned over and when its sand runs out; 0 until the first turn. */
export const hourglass = defineDocument({
  title: s.text({ description: "The title shown in the window." }),
  start: s.number({ min: 0, description: "When the countdown started, in epoch milliseconds." }),
  end: s.number({ min: 0, description: "When the countdown ends, in epoch milliseconds." }),
});
export type Hourglass = Value<typeof hourglass.descriptor>;
export default hourglass;
