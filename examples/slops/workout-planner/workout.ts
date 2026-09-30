import type { Exercise } from "./schema";

export function count(value: number, min = 1, max = 12) {
  return Math.max(min, Math.min(max, Number.isFinite(value) ? Math.round(value) : min));
}

export function sets(ex: Exercise) {
  return count(ex.sets);
}

export function checked(ex: Exercise): number[] {
  const values = ex.completedSetIndices;
  return [...new Set(values.filter((index) => Number.isInteger(index) && index >= 0 && index < sets(ex)))].sort((a, b) => a - b);
}

export function complete(ex: Exercise) {
  return checked(ex).length === sets(ex);
}

export function toggled(ex: Exercise, index: number) {
  const values = checked(ex);
  const added = !values.includes(index);
  const indices = added ? [...values, index].sort((a, b) => a - b) : values.filter((item) => item !== index);
  return { indices, added };
}

export function nextSet(ex: Exercise) {
  const values = checked(ex);
  return Array.from({ length: sets(ex) }, (_, index) => index).find((index) => !values.includes(index));
}

export function nextExercise(exercises: readonly Exercise[], after?: string) {
  const start = exercises.findIndex((ex) => ex.$id === after);
  for (let offset = 1; offset <= exercises.length; offset += 1) {
    const ex = exercises[(start + offset) % exercises.length];
    if (ex && !complete(ex)) return ex.$id;
  }
  return null;
}

export function remaining(deadline: number, now: number) {
  return Math.max(0, Math.ceil((deadline - now) / 1000));
}

export function formatTime(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
