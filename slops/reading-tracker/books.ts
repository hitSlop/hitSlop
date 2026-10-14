import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import type { statuses } from "./schema";

export type Library = Value<typeof schema.descriptor>;
export type Status = (typeof statuses)[number];
type Edit = Tx<typeof schema>;

/** The reading list and the export derive their counts from the same snapshot. */
export function readingView(data: Library) {
  return {
    total: data.books.length,
    read: data.books.filter((book) => book.status === "Read").length,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addBook(tx: Edit, input: { title: string; author: string; status: Status }) {
  const title = input.title.trim();
  if (!title) refuse("Enter a book title.");
  return tx.fields.books.insert({
    title,
    author: input.author.trim() || "Unknown Author",
    status: input.status,
  });
}
export function setRating(tx: Edit, book: RowRef, rating: number) {
  if (!Number.isInteger(rating) || rating < 1 || rating > 5) refuse("Rate a book from 1 to 5 stars.");
  tx.fields.books.item(book).rating.set(rating);
}
export function setStatus(tx: Edit, book: RowRef, status: Status) {
  tx.fields.books.item(book).status.set(status);
}
export function setNotes(tx: Edit, book: RowRef, notes: string) {
  tx.fields.books.item(book).notes.set(notes);
}
export function removeBook(tx: Edit, book: RowRef) {
  tx.fields.books.remove(book);
}
