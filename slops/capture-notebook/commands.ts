import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addNote, removeEntry, rename, setCaption } from "./notebook";

// Commands cannot capture or carry file bytes: photos and recordings are added in the
// window (or with `slop batch --attach`). These are the notebook's document verbs.
export default defineCommands(doc, {
  rename: {
    description: "Change the notebook's name shown at the top of the page.",
    args: { title: s.string({ description: "The new name." }) },
    run: ({ tx }, { title }) => rename(tx, title),
  },
  addNote: {
    description: "Write a text note into the notebook, stamped now, and return its stable row ID.",
    args: { text: s.string({ description: "The note's text." }) },
    run: ({ tx, now }, { text }) => addNote(tx, text, new Date(now).toISOString()),
  },
  setCaption: {
    description: "Replace the caption of a photo or recording, or the text of a note, by its $id.",
    args: {
      entry: s.string({ description: "The entry's $id." }),
      caption: s.string({ description: "The new caption; empty clears a photo or recording caption." }),
    },
    run: ({ tx, current }, { entry, caption }) => setCaption(tx, current, entry, caption),
  },
  removeEntry: {
    description: "Remove a photo, recording or note by its $id; a removed file is dropped from the document when it closes.",
    args: { entry: s.string({ description: "The entry's $id." }) },
    run: ({ tx, current }, { entry }) => removeEntry(tx, current, entry),
  },
});
