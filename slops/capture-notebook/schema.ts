import { defineDocument, s } from "hitslop";

export const kinds = ["photo", "audio", "note"] as const;

export default defineDocument({
  title: s.text({ description: "The notebook's name, shown at the top of the page." }),
  entries: s.list(s.object({
    kind: s.enum(kinds, { description: "photo, audio or note." }),
    at: s.string({ description: "When it was captured or written, as an ISO 8601 timestamp." }),
    caption: s.text({ description: "A caption for a photo or recording, or the text of a note." }),
    file: s.optional(s.object({
      id: s.string({ description: "The attachment ID." }),
      name: s.string(),
      mimeType: s.string(),
      byteLength: s.integer({ min: 0 }),
    }), { description: "The kept photo or recording (an attachment reference). Notes have none." }),
    width: s.optional(s.integer({ min: 1, description: "Photo width in pixels." })),
    height: s.optional(s.integer({ min: 1, description: "Photo height in pixels." })),
    seconds: s.optional(s.number({ min: 0, description: "Recording length in seconds." })),
  })),
}, { initial: {
  title: "Plot 14, autumn",
  entries: [
    { kind: "note", at: "2026-10-04T08:40:00.000Z", caption: "Runner beans are finished. Pull the canes on Sunday and photograph the bed before mulching." },
    { kind: "note", at: "2026-10-04T08:52:00.000Z", caption: "Leaf spots on the courgettes again. Take a close photo every visit so we can compare." },
    { kind: "note", at: "2026-10-04T09:10:00.000Z", caption: "Ask Pat at the gate about sharing the water butt, and record what she says." },
  ],
} });
