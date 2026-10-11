import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { redateFrame, removeFrame, rename, setNote } from "./frames";

// Photos are added from the window (camera or file). Commands cannot carry file bytes;
// an agent adds a frame with `slop batch --attach` and an insert into `frames`.
const day = (description: string) => s.string({ minLength: 10, maxLength: 10, description });

export default defineCommands(doc, {
  rename: {
    description: "Change the heading that says what the frames follow.",
    args: { title: s.string({ minLength: 1 }) },
    run: ({ tx }, { title }) => rename(tx, title),
  },
  setNote: {
    description: "Write the caption on one day's frame and return the frame's $id.",
    args: { date: day("The frame's local day, YYYY-MM-DD."), note: s.string() },
    run: ({ tx, current }, { date, note }) => setNote(tx, current, date, note),
  },
  redateFrame: {
    description: "Move a frame filed under the wrong day to another day that has no frame, and return its $id.",
    args: { date: day("The frame's current day, YYYY-MM-DD."), to: day("The new day, YYYY-MM-DD.") },
    run: ({ tx, current }, { date, to }) => redateFrame(tx, current, date, to),
  },
  removeFrame: {
    description: "Remove one day's frame; its photo leaves the file when the document closes.",
    args: { date: day("The frame's local day, YYYY-MM-DD.") },
    run: ({ tx, current }, { date }) => removeFrame(tx, current, date),
  },
});
