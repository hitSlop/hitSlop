import { defineDocument, s } from "hitslop";

export const statuses = ["To Read", "Reading", "Read"] as const;

export default defineDocument({
  memberName: s.text(),
  memberSince: s.text(),
  books: s.list(s.object({
    title: s.text(),
    author: s.text(),
    rating: s.integer({ min: 0, max: 5, default: 0 }),
    status: s.enum(statuses, { default: "To Read" }),
    notes: s.optional(s.text()),
  })),
}, { initial: {
  memberName: "Jamie Park",
  memberSince: "Member since 2023",
  books: [
    { title: "Atomic Habits", author: "James Clear", rating: 5, status: "Read" },
    { title: "The Alchemist", author: "P. Coelho", rating: 5, status: "Read" },
    { title: "Sapiens", author: "Yuval Noah Harari", rating: 4, status: "Reading" },
    { title: "Deep Work", author: "Cal Newport", rating: 5, status: "To Read" },
    { title: "The Creative Act", author: "Rick Rubin", rating: 4, status: "To Read" },
  ],
} });
