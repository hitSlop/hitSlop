import { defineDocument, s } from "hitslop";

export default defineDocument({
  displayName: s.text({ description: "The active pet's name, from its pet.json." }),
  package: s.optional(s.object({
    id: s.string({ description: "The imported pet ZIP's attachment ID." }),
    name: s.string(),
    mimeType: s.string(),
    byteLength: s.integer({ min: 0 }),
  }), { description: "The imported pet ZIP; absent while the bundled Bubu is the pet." }),
}, { initial: {
  displayName: "Bubu",
} });
