import { refuse, type Tx } from "hitslop";
import type doc from "./schema";

export type Board = typeof doc.current;
export type Lane = Board["lanes"][number];
export type Card = Board["cards"][number];
type Edit = Tx<typeof doc>;

/** The editor and export read lanes, counts and limits from the same snapshot. */
export function boardView(board: Board) {
  const grouped = new Map<string, Card[]>();
  for (const lane of board.lanes) grouped.set(lane.laneKey, []);
  for (const card of board.cards) grouped.get(card.laneKey)?.push(card);
  const cardsFor = (laneKey: string): Card[] => grouped.get(laneKey) ?? [];
  const isOverLimit = (lane: Lane) =>
    lane.limit !== undefined && lane.limit > 0 && cardsFor(lane.laneKey).length > lane.limit;
  const isDone = (laneKey: string) => board.doneLaneKey === laneKey;
  const doneCount = board.doneLaneKey ? cardsFor(board.doneLaneKey).length : 0;
  return {
    cardsFor, isOverLimit, isDone, doneCount,
    openCount: board.cards.length - doneCount,
    overLimitCount: board.lanes.filter(isOverLimit).length,
  };
}

/** The lane that takes a removed lane's orders: its left neighbour, or the next lane for the first. */
export function refugeFor(board: Board, laneKey: string): Lane | undefined {
  const index = board.lanes.findIndex((lane) => lane.laneKey === laneKey);
  return board.lanes[index === 0 ? 1 : index - 1];
}

export const pad = (value: number) => String(value).padStart(2, "0");

/** A lane key from one random draw; the page passes a UUID instead. */
export function laneKeyFrom(draw: number): string {
  return `lane-${Math.floor(draw * 2 ** 32).toString(36)}`;
}

function laneOf(board: Board, laneKey: string): Lane {
  const lane = board.lanes.find((item) => item.laneKey === laneKey);
  if (lane) return lane;
  return refuse("Choose a lane on this board.");
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addCard(tx: Edit, board: Board, input: { laneKey: string; title: string; note: string; tag: string }) {
  const title = input.title.trim();
  if (!title) refuse("Enter a work order title.");
  laneOf(board, input.laneKey);
  return tx.fields.cards.insert({ laneKey: input.laneKey, title, note: input.note.trim(), tag: input.tag.trim() });
}

/** Moves a card to the end of a lane, so concurrent moves merge. */
export function moveCard(tx: Edit, board: Board, cardId: string, laneKey: string) {
  if (!board.cards.some((card) => card.$id === cardId)) refuse("That work order is no longer on the board.");
  laneOf(board, laneKey);
  const last = board.cards.filter((card) => card.laneKey === laneKey && card.$id !== cardId).at(-1);
  tx.fields.cards.item(cardId).laneKey.set(laneKey);
  if (last) tx.fields.cards.move(cardId, { after: last.$id });
}

export function removeCard(tx: Edit, cardId: string) {
  tx.fields.cards.remove(cardId);
}

export function addLane(tx: Edit, board: Board, input: { title: string; laneKey: string }) {
  const title = input.title.trim();
  if (!title) refuse("Enter a lane name.");
  if (board.lanes.some((lane) => lane.laneKey === input.laneKey)) refuse("That lane already exists.");
  return tx.fields.lanes.insert({ laneKey: input.laneKey, title });
}

/** Moves the lane's orders to its refuge (after the refuge's last order), then removes it. */
export function removeLane(tx: Edit, board: Board, laneKey: string) {
  const lane = laneOf(board, laneKey);
  const refuge = refugeFor(board, laneKey);
  if (!refuge) refuse("A board needs at least one other lane.");
  const stranded = board.cards.filter((card) => card.laneKey === laneKey);
  // Each moved card goes after the previous one, so the lane keeps its order.
  let after = board.cards.filter((card) => card.laneKey === refuge.laneKey).at(-1)?.$id;
  for (const card of stranded) {
    tx.fields.cards.item(card.$id).laneKey.set(refuge.laneKey);
    if (after) tx.fields.cards.move(card.$id, { after });
    after = card.$id;
  }
  tx.fields.lanes.remove(lane.$id);
  if (board.doneLaneKey === laneKey) tx.fields.doneLaneKey.clear();
  return stranded.length;
}

/** An empty laneKey clears the completed lane. */
export function setDoneLane(tx: Edit, board: Board, laneKey: string) {
  if (laneKey === "") {
    tx.fields.doneLaneKey.clear();
    return;
  }
  laneOf(board, laneKey);
  tx.fields.doneLaneKey.set(laneKey);
}

/** An undefined limit removes the limit; numbers are clamped to 0 through 999. */
export function setLaneLimit(tx: Edit, board: Board, laneKey: string, limit: number | undefined) {
  const lane = laneOf(board, laneKey);
  const field = tx.fields.lanes.item(lane.$id).limit;
  if (limit === undefined) field.clear();
  else field.set(Math.min(999, Math.max(0, Math.floor(limit))));
}
