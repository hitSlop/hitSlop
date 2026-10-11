import { defineDocument, s } from "hitslop";

export default defineDocument({
  decks: s.list(s.object({
    deckKey: s.string(),
    name: s.text(),
    cards: s.list(s.object({
      front: s.text(),
      back: s.text(),
      box: s.integer({ min: 1, max: 4, default: 1 }),
      lastReviewed: s.optional(s.string()),
    })),
  })),
}, { initial: {
  decks: [
    {
      deckKey: "d-type",
      name: "Type Terms",
      cards: [
        { front: "Kerning", back: "The space between a specific pair of letters.", box: 1 },
        { front: "Leading", back: "Vertical space from one baseline to the next.", box: 1 },
        { front: "X-height", back: "The height of lowercase letters, excluding ascenders.", box: 2 },
        { front: "Ligature", back: "Two or more letters drawn as a single glyph.", box: 2 },
        { front: "Counter", back: "The enclosed or partially enclosed interior of a letter.", box: 3 },
        { front: "Serif", back: "A small finishing stroke on the end of a letter stem.", box: 4 },
      ],
    },
  ],
} });
