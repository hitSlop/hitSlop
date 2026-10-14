import { defineCommands } from "hitslop/commands";
import { refuse, s } from "hitslop";
import schema from "./schema";
import { addTypedNote, removeNote, renamePad, setLocale, setTranscript } from "./notes";

export default defineCommands(schema, {
  addNote: {
    description: "Add a typed note without a recording and return its stable row ID. Recordings are added through the page, or with `batch --attach` storing an attachment reference in `clip`.",
    args: {
      text: s.string({ description: "The note's text." }),
      recordedAt: s.optional(s.string({ description: "ISO 8601 instant; now when absent." })),
    },
    run: ({ tx, now }, { text, recordedAt }) => {
      if (!text.trim()) refuse("Enter the note's text.");
      return addTypedNote(tx, text.trim(), recordedAt ?? new Date(now).toISOString()).id;
    },
  },
  setTranscript: {
    description: "Replace a note's text, keeping its recording.",
    args: {
      note: s.string({ description: "The note's $id." }),
      text: s.string(),
    },
    run: ({ tx }, { note, text }) => setTranscript(tx, note, text),
  },
  removeNote: {
    description: "Delete a note; its recording leaves the file when the document closes.",
    args: { note: s.string({ description: "The note's $id." }) },
    run: ({ tx }, { note }) => removeNote(tx, note),
  },
  setLanguage: {
    description: "Set the speech locale used for transcription, such as en-US; an empty string uses the device's language.",
    args: { locale: s.string() },
    run: ({ tx }, { locale }) => setLocale(tx, locale),
  },
  renamePad: {
    description: "Rename the pad.",
    args: { title: s.string() },
    run: ({ tx }, { title }) => renamePad(tx, title),
  },
});
