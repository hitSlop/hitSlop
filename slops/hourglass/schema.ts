import { defineDocument, s } from "hitslop";
/** What the glass counts down to. `start` and `end` are epoch milliseconds: when it was
 * turned over and when its sand runs out; 0 until the first turn. */
const hourglass = defineDocument({
  title: s.text({ description: "The title shown in the window." }),
  start: s.number({ min: 0, description: "When the countdown started, in epoch milliseconds." }),
  end: s.number({ min: 0, description: "When the countdown ends, in epoch milliseconds." }),
}, { initial: { title: "", start: 0, end: 0 } });
export default hourglass;
