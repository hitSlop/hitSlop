import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";
export type Recipe = typeof doc.current;
export type Step = Recipe["steps"][number];
export type RowKind = "ingredient" | "step";
type Edit = Tx<typeof doc>;

/** Seconds on a step's timer; a step without minutes has no timer. */
export function stepSeconds(step: Step): number {
  return Math.max(0, Math.round(Number(step.minutes) || 0) * 60);
}
export function stepTitle(step: Step, index: number): string {
  return step.title.trim() || `Step ${index + 1}`;
}

/** The cook view and the export derive their titles and counts from the same snapshot. */
export function recipeView(data: Recipe) {
  return {
    title: data.title.trim() || "Untitled recipe",
    stepCount: data.steps.length,
    timedSteps: data.steps.filter((step) => stepSeconds(step) > 0).length,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addIngredient(tx: Edit, text: string) {
  const trimmed = text.trim();
  if (!trimmed) refuse("Enter an ingredient.");
  return tx.fields.ingredients.insert({ text: trimmed });
}
export function setChecked(tx: Edit, ingredient: RowRef, checked: boolean) {
  tx.fields.ingredients.item(ingredient).checked.set(checked);
}
export function addStep(tx: Edit, input: { title: string; text: string }) {
  return tx.fields.steps.insert({ title: input.title, text: input.text });
}
/** `undefined` clears the timer. */
export function setStepTimer(tx: Edit, step: RowRef, minutes: number | undefined) {
  const timer = tx.fields.steps.item(step).minutes;
  if (minutes === undefined) timer.clear();
  else timer.set(minutes);
}
export function moveRow(tx: Edit, data: Recipe, kind: RowKind, row: string, direction: "up" | "down") {
  const list: readonly { readonly $id: string }[] = kind === "ingredient" ? data.ingredients : data.steps;
  const index = list.findIndex((item) => item.$id === row);
  const neighbor = list[direction === "up" ? index - 1 : index + 1];
  if (index < 0 || !neighbor) return;
  const target = kind === "ingredient" ? tx.fields.ingredients : tx.fields.steps;
  target.move(row, direction === "up" ? { before: neighbor.$id } : { after: neighbor.$id });
}
export function removeRow(tx: Edit, kind: RowKind, row: RowRef) {
  if (kind === "ingredient") tx.fields.ingredients.remove(row);
  else tx.fields.steps.remove(row);
}
