import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  prompt: "How are you, really?",
  entries: [
    { day: "TODAY", mood: 4, energy: 4, note: "Clear head after a quiet morning." },
    { day: "YESTERDAY", mood: 5, energy: 5, note: "Deep work clicked." },
    { day: "TUE 26", mood: 3, energy: 3, note: "A little stretched, still moving." },
  ],
} satisfies Input<typeof schema.fields.node>;
