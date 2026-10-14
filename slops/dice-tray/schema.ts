import { defineDocument, s } from "hitslop";

/** The six dice a tabletop tray holds, in the order the rack shows them. */
export const kinds = ["d4", "d6", "d8", "d10", "d12", "d20"] as const;
export type Kind = (typeof kinds)[number];
/** At most this many dice in one roll (they have to fit on the felt). */
export const maxDice = 12;
/** The oldest roll is dropped once the history holds this many. */
export const maxRolls = 60;
export const maxModifier = 99;

const count = s.integer({ min: 0, max: maxDice, default: 0 });

export default defineDocument({
  title: s.text({ description: "The table or game this tray belongs to." }),
  // The dice the next roll uses, as a count per kind, plus a flat modifier.
  pool: s.object({ d4: count, d6: count, d8: count, d10: count, d12: count, d20: count }),
  modifier: s.integer({ min: -maxModifier, max: maxModifier, default: 0 }),
  // Every roll as it came up, oldest first. Values are drawn from an explicit random source
  // and stored; `seed` only drives the tumble animation that lands on them.
  rolls: s.list(s.object({
    at: s.string({ description: "When it was rolled, as an ISO 8601 timestamp." }),
    seed: s.integer({ min: 0, max: 2147483647, description: "Seeds the tumble and resting layout; never the values." }),
    modifier: s.integer({ min: -maxModifier, max: maxModifier, default: 0 }),
    dice: s.list(s.object({
      kind: s.enum(kinds),
      value: s.integer({ min: 1, max: 20 }),
    })),
  })),
}, { initial: {
  title: "Thursday night table",
  pool: { d20: 1, d4: 1 },
  modifier: 5,
  rolls: [
    { at: "2026-10-08T19:12:00.000Z", seed: 90211, modifier: 2, dice: [{ kind: "d20", value: 11 }] },
    { at: "2026-10-08T19:26:00.000Z", seed: 51873, modifier: 4, dice: [{ kind: "d8", value: 6 }, { kind: "d6", value: 5 }, { kind: "d6", value: 2 }, { kind: "d6", value: 6 }] },
    { at: "2026-10-08T19:41:00.000Z", seed: 7741, modifier: 0, dice: [{ kind: "d12", value: 9 }] },
    { at: "2026-10-08T19:58:00.000Z", seed: 330197, modifier: 1, dice: [{ kind: "d4", value: 3 }, { kind: "d4", value: 4 }, { kind: "d4", value: 1 }] },
    { at: "2026-10-08T20:07:00.000Z", seed: 18420, modifier: 5, dice: [{ kind: "d20", value: 17 }, { kind: "d4", value: 3 }] },
  ],
} });
