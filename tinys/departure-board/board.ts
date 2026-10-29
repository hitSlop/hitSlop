import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";

export type Board = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export const statuses = ["on-time", "boarding", "delayed", "gone"] as const;
export type Status = (typeof statuses)[number];
export const maxRows = 8;
export const widths = { title: 24, time: 5, text: 14, status: 8 } as const;

export const statusLabel: Record<Status, string> = {
  "on-time": "ON TIME",
  boarding: "BOARDING",
  delayed: "DELAYED",
  gone: "DEPARTED",
};

export const nextStatus = (status: Status): Status => statuses[(statuses.indexOf(status) + 1) % statuses.length]!;

/** Board text is upper case and padded to its column, one character per flap. */
export const cells = (text: string, width: number): string[] => [...text.toUpperCase().padEnd(width).slice(0, width)];

export const clean = (text: string, width: number): string => text.toUpperCase().trim().slice(0, width);

/** The live board and the export lay out the same heading, rows and flap cells. */
export function boardView(data: Board) {
  return {
    heading: clean(data.title, widths.title) || "DEPARTURES",
    full: data.rows.length >= maxRows,
    rows: data.rows.map((row) => ({
      row,
      label: `${row.time} ${row.text}: ${statusLabel[row.status]}`,
      groups: [
        { kind: "time", cells: cells(row.time, widths.time) },
        { kind: "text", cells: cells(row.text, widths.text) },
        { kind: "status", cells: cells(statusLabel[row.status], widths.status) },
      ],
    })),
  };
}

function stopIn(current: Board, stop: RowRef) {
  const id = typeof stop === "string" ? stop : stop.$id;
  const row = current.rows.find((candidate) => candidate.$id === id);
  if (!row) refuse("That stop is not on the board.");
  return row;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setTitle(tx: Edit, title: string) {
  tx.fields.title.set(clean(title, widths.title));
}
export function addStop(tx: Edit, current: Board, time: string, text: string, status: Status = "on-time") {
  if (current.rows.length >= maxRows) refuse(`The board is full: it holds ${maxRows} stops.`);
  const name = clean(text, widths.text);
  if (!name) refuse("Enter where the stop goes.");
  return tx.fields.rows.insert({ time: clean(time, widths.time), text: name, status });
}
export function editStop(tx: Edit, current: Board, stop: RowRef, change: { time?: string; text?: string }) {
  if (change.time === undefined && change.text === undefined) refuse("Give a new time or name for the stop.");
  const row = tx.fields.rows.item(stopIn(current, stop));
  if (change.time !== undefined) row.time.set(clean(change.time, widths.time));
  if (change.text !== undefined) row.text.set(clean(change.text, widths.text));
}
export function setStatus(tx: Edit, current: Board, stop: RowRef, status: Status) {
  tx.fields.rows.item(stopIn(current, stop)).status.set(status);
}
/** Advances a stop to the next status (on time, boarding, delayed, departed, then around). */
export function cycleStatus(tx: Edit, current: Board, stop: RowRef): Status {
  const row = stopIn(current, stop);
  const status = nextStatus(row.status);
  tx.fields.rows.item(row).status.set(status);
  return status;
}
export function removeStop(tx: Edit, current: Board, stop: RowRef) {
  tx.fields.rows.remove(stopIn(current, stop));
}
