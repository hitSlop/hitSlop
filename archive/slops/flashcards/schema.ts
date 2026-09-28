import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  decks: s.list(s.object({
    deckKey: s.string(),
    name: s.text(),
    cards: s.list(s.object({
      front: s.text(),
      back: s.text(),
      box: s.integer({ min: 1, max: 4 }),
      lastReviewed: s.optional(s.string()),
    })),
  })),
  selectedDeckId: s.string(),
  selectedBox: s.integer({ min: 0, max: 4 }),
  cardIndex: s.integer({ min: 0 }),
});

export type Flashcards = Value<typeof schema.fields.node>;
export type Deck = Flashcards["decks"][number];
export type Flashcard = Deck["cards"][number];
export default schema;
