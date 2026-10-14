import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import type { lists } from "./schema";

export type Choices = Value<typeof schema.descriptor>;
export type ListKey = (typeof lists)[number];
type Edit = Tx<typeof schema>;

/** The editor derives its lists and commitment state from one snapshot. */
export function choicePointView(data: Choices) {
  return {
    lists: {
      hooks: data.hooks,
      awayMoves: data.awayMoves,
      towardsMoves: data.towardsMoves,
      helpers: data.helpers,
    },
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addPoint(tx: Edit, list: ListKey, text: string) {
  const clean = text.trim();
  if (!clean) refuse("Enter text first.");
  return tx.fields[list].insert({ text: clean });
}
export function removePoint(tx: Edit, list: ListKey, id: string) {
  tx.fields[list].remove(id);
}
export function setNextMove(tx: Edit, move: string, when: string) {
  tx.fields.nextMove.set(move);
  tx.fields.nextWhen.set(when);
}
export function setNextDone(tx: Edit, done: boolean) {
  tx.fields.nextDone.set(done);
}
