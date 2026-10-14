import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { statuses } from "./schema";
import { addBook, removeBook, setNotes, setRating, setStatus } from "./books";

export default defineCommands(schema, {
  addBook: {
    description: "Add a book to the reading list and return its stable row ID; a blank author becomes Unknown Author.",
    args: {
      title: s.string({ description: "The book's title." }),
      author: s.string({ description: "The author, or blank." }),
      status: s.enum(statuses, { description: "Where the book stands." }),
    },
    run: ({ tx }, args) => addBook(tx, args),
  },
  setRating: {
    description: "Give a book a rating from 1 to 5 stars by its $id.",
    args: {
      book: s.string({ description: "The book's $id." }),
      rating: s.integer({ min: 1, max: 5, description: "Stars, 1 to 5." }),
    },
    run: ({ tx }, { book, rating }) => setRating(tx, book, rating),
  },
  setStatus: {
    description: "Set a book's status by its $id.",
    args: {
      book: s.string({ description: "The book's $id." }),
      status: s.enum(statuses, { description: "To Read, Reading or Read." }),
    },
    run: ({ tx }, { book, status }) => setStatus(tx, book, status),
  },
  setNotes: {
    description: "Replace a book's notes by its $id; blank text clears them.",
    args: {
      book: s.string({ description: "The book's $id." }),
      notes: s.string({ description: "The notes text." }),
    },
    run: ({ tx }, { book, notes }) => setNotes(tx, book, notes),
  },
  removeBook: {
    description: "Remove a book from the reading list by its $id.",
    args: { book: s.string({ description: "The book's $id." }) },
    run: ({ tx }, { book }) => removeBook(tx, book),
  },
});
