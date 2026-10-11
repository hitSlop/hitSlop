import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { koiPatterns } from "./schema";
import { addKoi, feedKoi, releaseKoi, renameKoi, repaintPond } from "./shared";

export default defineCommands(doc, {
  addKoi: {
    description: "Add a koi (at most nine) with the given or first free name and a given or random pattern, and return its stable row ID.",
    args: {
      name: s.optional(s.string({ description: "Up to 24 characters; defaults to the first unused pond name." })),
      pattern: s.optional(s.enum(koiPatterns, { description: "Defaults to a random pattern." })),
    },
    run: ({ tx, current, random }, { name, pattern }) => addKoi(tx, current, random, { name, pattern }),
  },
  renameKoi: {
    description: "Rename a koi.",
    args: { koi: s.string({ description: "The koi's $id." }), name: s.string({ description: "Up to 24 characters." }) },
    run: ({ tx, current }, { koi, name }) => renameKoi(tx, current, koi, name),
  },
  releaseKoi: {
    description: "Release a koi from the pond and return the name it had.",
    args: { koi: s.string({ description: "The koi's $id." }) },
    run: ({ tx, current }, { koi }) => releaseKoi(tx, current, koi),
  },
  feed: {
    description: "Count one feeding of the koi and return the new total.",
    args: {},
    run: ({ tx, current }) => feedKoi(tx, current),
  },
  repaint: {
    description: "Repaint the pond with a new watercolor seed (random when omitted) and return the seed.",
    args: { seed: s.optional(s.integer({ min: 0, max: 999999, description: "A specific painting seed." })) },
    run: ({ tx, current, random }, { seed }) => repaintPond(tx, current, random, seed),
  },
});
