import { defineDocument, s } from "hitslop";
// Every descriptor kind and option, so a frozen build of this app exercises them all.
export default defineDocument({
  title: s.text(),
  version: s.string(),
  epoch: s.integer(),
  done: s.boolean(),
  hits: s.counter(),
  rows: s.list(s.object({ text: s.text(), done: s.boolean(), tags: s.list(s.string()), notes: s.record(s.string()) })),
  label: s.string({ maxLength: 40 }),
  ratio: s.number({ min: 0, max: 1 }),
  count: s.integer({ min: 0, max: 100 }),
  lane: s.enum(["todo", "doing", "done"]),
  note: s.optional(s.string()),
  memo: s.optional(s.text()),
  place: s.optional(s.object({ name: s.string(), visits: s.integer({ min: 0 }) })),
  settings: s.object({ volume: s.integer({ min: 0, max: 10 }), muted: s.boolean() }),
  colors: s.list(s.string()),
  checkins: s.record(s.integer({ min: 0 })),
  cells: s.record(s.object({ input: s.text(), width: s.optional(s.integer({ min: 0 })) })),
  attachment: s.optional(s.string()),
  photo: s.optional(s.string()),
});
