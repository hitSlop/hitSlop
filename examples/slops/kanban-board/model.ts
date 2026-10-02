import type { Board, Card, Lane } from "./schema";

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

export const pad = (value: number) => String(value).padStart(2, "0");
