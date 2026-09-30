import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  memberName: "Jamie Park",
  memberSince: "Member since 2023",
  books: [
    { title: "Atomic Habits", author: "James Clear", rating: 5, status: "Read" },
    { title: "The Alchemist", author: "P. Coelho", rating: 5, status: "Read" },
    { title: "Sapiens", author: "Yuval Noah Harari", rating: 4, status: "Reading" },
    { title: "Deep Work", author: "Cal Newport", rating: 5, status: "To Read" },
    { title: "The Creative Act", author: "Rick Rubin", rating: 4, status: "To Read" },
  ],
} satisfies Input<typeof schema.fields.node>;
