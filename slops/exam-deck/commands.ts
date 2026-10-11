import { defineCommands } from "hitslop/commands";
import { refuse, s } from "hitslop";
import doc from "./schema";
import {
  addCard, deckStatus, localDay, removeCard, resetProgress, reviewCard, setBudget, setExamDate, setRetention,
} from "./deck";

const card = s.string({ description: "The card's $id." });
const utcOffsetMinutes = s.optional(s.integer({
  min: -840, max: 840,
  description: "Minutes east of UTC for deciding which day is today (e.g. -300 for New York in winter). Default 0.",
}));

export default defineCommands(doc, {
  status: {
    description: "Read today's card count against the budget, the load forecast up to 14 days or the exam, and the estimated readiness by the exam. Changes nothing.",
    args: { utcOffsetMinutes },
    run: ({ current, now }, args) => deckStatus(current, now, args.utcOffsetMinutes ?? 0),
  },
  addCard: {
    description: "Add a new card at the end of the deck and return its $id.",
    args: { front: s.string({ description: "The prompt." }), back: s.string({ description: "The answer." }) },
    run: ({ tx }, { front, back }) => addCard(tx, front, back),
  },
  reviewCard: {
    description: "Answer a card now (1 Again, 2 Hard, 3 Good, 4 Easy), schedule it with FSRS and log the review. Returns the card's new state and due time.",
    args: { card, rating: s.integer({ min: 1, max: 4, description: "1 Again, 2 Hard, 3 Good, 4 Easy." }) },
    run: ({ tx, current, now }, args) => {
      const next = reviewCard(tx, current, args.card, args.rating, now);
      return { state: next.state, due: next.due ?? null, scheduledDays: next.scheduledDays, reps: next.reps, lapses: next.lapses };
    },
  },
  setExamDate: {
    description: "Set the exam date (YYYY-MM-DD, today or later), or leave date out to clear it and assume an exam 28 days out.",
    args: { date: s.optional(s.string({ description: "The exam's local date, YYYY-MM-DD." })), utcOffsetMinutes },
    run: ({ tx, now }, args) => setExamDate(tx, args.date, localDay(now, args.utcOffsetMinutes ?? 0)),
  },
  setRetention: {
    description: "Set the target retention, the chance of recalling a card when it comes due (0.70 to 0.99). Higher means more reviews.",
    args: { retention: s.number({ description: "Between 0.70 and 0.99." }) },
    run: ({ tx }, { retention }) => setRetention(tx, retention),
  },
  setBudget: {
    description: "Change the daily study budget. Give any of dailyMinutes, secondsPerCard and newPerDay, or automaticNew: true to pace new cards automatically.",
    args: {
      dailyMinutes: s.optional(s.integer({ description: "Minutes per day, 1 to 600." })),
      secondsPerCard: s.optional(s.integer({ description: "Assumed seconds per review, 3 to 600." })),
      newPerDay: s.optional(s.integer({ description: "Fixed new cards per day, 1 to 1000." })),
      automaticNew: s.optional(s.boolean({ description: "True spreads the remaining new cards evenly until two days before the exam." })),
    },
    run: ({ tx }, args) => {
      if (args.automaticNew && args.newPerDay !== undefined) {
        refuse("Give newPerDay or automaticNew, not both.");
      }
      setBudget(tx, {
        dailyMinutes: args.dailyMinutes,
        secondsPerCard: args.secondsPerCard,
        newPerDay: args.automaticNew ? null : args.newPerDay,
      });
    },
  },
  removeCard: {
    description: "Remove a card and its review history.",
    args: { card },
    run: ({ tx, current }, args) => removeCard(tx, current, args.card),
  },
  resetProgress: {
    description: "Return every card to new and clear the review log. Returns the number of cards reset.",
    args: {},
    run: ({ tx, current }) => resetProgress(tx, current),
  },
});
