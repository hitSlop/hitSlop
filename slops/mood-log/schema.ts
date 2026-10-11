import { defineDocument, s } from "hitslop";

export default defineDocument({
  prompt: s.text(),
  entries: s.list(s.object({
    day: s.string(),
    mood: s.integer({ min: 1, max: 5 }),
    energy: s.integer({ min: 1, max: 5 }),
    note: s.text(),
  })),
}, { initial: {
  prompt: "How are you, really?",
  entries: [
    { day: "TODAY", mood: 4, energy: 4, note: "Clear head after a quiet morning." },
    { day: "YESTERDAY", mood: 5, energy: 5, note: "Deep work clicked." },
    { day: "TUE 26", mood: 3, energy: 3, note: "A little stretched, still moving." },
  ],
} });
