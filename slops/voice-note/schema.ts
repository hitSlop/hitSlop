import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text({ description: "The pad's name, shown at the top." }),
  locale: s.string({
    default: "",
    maxLength: 35,
    description: "Speech locale for transcription, such as en-US. Empty uses the device's own language.",
  }),
  notes: s.list(s.object({
    recordedAt: s.string({ description: "When the note was made, as an ISO 8601 instant." }),
    durationMs: s.integer({ min: 0, default: 0, description: "Recording length in milliseconds; 0 for a typed note." }),
    clip: s.optional(s.object({
      id: s.string(),
      byteLength: s.integer({ min: 0 }),
      name: s.string(),
      mimeType: s.string(),
    }, { description: "The recording, an attachment reference as `attachments import` or `attachments ref` prints it. Absent for a typed note." })),
    transcript: s.text({ description: "The note's text: a transcript, edited by hand, or typed." }),
    transcribed: s.optional(s.object({
      locale: s.string(),
      at: s.string({ description: "ISO 8601 instant of the last on-device transcription." }),
    }, { description: "The last on-device transcription. Absent when the text was never transcribed." })),
  })),
}, {
  initial: {
    title: "Pocket dictation",
    notes: [
      {
        recordedAt: "2026-10-09T16:40:00.000Z",
        transcript: "Pick up oat milk, lemons and the good bread on the way home. Ask Sam whether Saturday still works for the bike ride, and if not, Sunday morning.",
      },
      {
        recordedAt: "2026-10-08T15:15:00.000Z",
        transcript: "Idea for the talk: open with the broken kettle story, then the three numbers, then the slide with the map. Keep it under twelve minutes.",
      },
      {
        recordedAt: "2026-10-07T17:00:00.000Z",
        transcript: "How this pad works: press the red button and talk. Stop and keep saves the recording here and transcribes it on-device. Click any text to fix a word. These first notes were typed, so they have no recording.",
      },
    ],
  },
});
