import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { addCard, addDeck, editCard, gradeCard, newDeckKey, removeCard, shuffleDeck } from "./study";

const deck = s.string({ description: "The deck's $id." });
const card = s.string({ description: "The card's $id." });

export default defineCommands(doc, {
  addDeck: {
    description: "Create an empty deck with the given name and return its stable row ID; a blank name is refused.",
    args: { name: s.string({ description: "The deck's name." }) },
    run: (ctx, { name }) => addDeck(ctx.tx, name, newDeckKey(() => ctx.random())),
  },
  addCard: {
    description: "Add a card to a deck in box 1 and return its stable row ID; a blank front or back is refused.",
    args: {
      deck,
      front: s.string({ description: "The question side." }),
      back: s.string({ description: "The answer side." }),
    },
    run: (ctx, args) => addCard(ctx.tx, ctx.current, args.deck, args.front, args.back),
  },
  gradeCard: {
    description: "Grade a card: a good answer moves it up one Leitner box, a miss moves it down one, and lastReviewed is stamped now. Returns the new box, 1 to 4.",
    args: {
      deck,
      card,
      good: s.boolean({ description: "True for Got it, false for Again." }),
    },
    run: (ctx, args) => gradeCard(ctx.tx, ctx.current, args.deck, args.card, args.good, ctx.now),
  },
  editCard: {
    description: "Replace a card's front and back text by its $id; blank text is refused.",
    args: {
      deck,
      card,
      front: s.string({ description: "The new question side." }),
      back: s.string({ description: "The new answer side." }),
    },
    run: (ctx, args) => editCard(ctx.tx, ctx.current, args.deck, args.card, args.front, args.back),
  },
  removeCard: {
    description: "Remove a card from a deck by its $id.",
    args: { deck, card },
    run: (ctx, args) => removeCard(ctx.tx, ctx.current, args.deck, args.card),
  },
  shuffleDeck: {
    description: "Shuffle the card order of a deck and return the number of cards shuffled.",
    args: { deck },
    run: (ctx, args) => shuffleDeck(ctx.tx, ctx.current, args.deck, () => ctx.random()),
  },
});
