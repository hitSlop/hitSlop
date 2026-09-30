import { defineDocument, s, type Value } from "@hitslop/document";

export const statuses = ["To Read", "Reading", "Read"] as const;

const schema = defineDocument({
  memberName: s.text(),
  memberSince: s.text(),
  books: s.list(s.object({
    title: s.text(),
    author: s.text(),
    rating: s.integer({ min: 0, max: 5 }),
    status: s.enum(statuses),
    notes: s.optional(s.text()),
  })),
});

export type ReadingTracker = Value<typeof schema.fields.node>;
export type Book = ReadingTracker["books"][number];
export default schema;
