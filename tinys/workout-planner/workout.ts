import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { restPresets } from "./schema";

export type Data = Value<typeof schema.descriptor>;
export type Exercise = Data["exercises"][number];
type Edit = Tx<typeof schema>;

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

/** The editor and its captures derive their totals from the same snapshot. */
export function workoutView(data: Data) {
  const total = data.exercises.reduce((sum, ex) => sum + sets(ex), 0);
  const done = data.exercises.reduce((sum, ex) => sum + checked(ex).length, 0);
  return { total, done, finished: total > 0 && done === total, ratio: total ? (done / total) * 100 : 0 };
}

export type ExerciseFields = { name: string; sets: number; reps: number; weight: string };

function clean(fields: ExerciseFields) {
  const name = fields.name.trim();
  if (!name) refuse("Give this exercise a name.");
  if (!Number.isFinite(fields.sets) || !Number.isFinite(fields.reps)) refuse("Enter a set and rep target.");
  return {
    name,
    sets: count(fields.sets),
    reps: count(fields.reps, 1, 50),
    weight: fields.weight.trim() || "Bodyweight",
  };
}

function find(data: Data, id: string) {
  const ex = data.exercises.find((item) => item.$id === id);
  if (!ex) refuse("No such exercise.");
  return ex;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addExercise(tx: Edit, fields: ExerciseFields) {
  return tx.fields.exercises.insert({ ...clean(fields), completedSetIndices: [] });
}

export function updateExercise(tx: Edit, data: Data, id: string, fields: ExerciseFields) {
  const original = find(data, id);
  const next = clean(fields);
  const row = tx.at(original);
  row.name.set(next.name);
  row.sets.set(next.sets);
  row.reps.set(next.reps);
  row.weight.set(next.weight);
  row.completedSetIndices.replace(checked(original).filter((index) => index < next.sets));
}

export function removeExercise(tx: Edit, data: Data, id: string) {
  find(data, id);
  tx.fields.exercises.remove(id);
}

/** Marks a set done or not done; returns true when it is now done. */
export function toggleSet(tx: Edit, data: Data, id: string, index: number) {
  const ex = find(data, id);
  if (!Number.isInteger(index) || index < 0 || index >= sets(ex)) refuse(`Set ${index + 1} does not exist.`);
  const next = toggled(ex, index);
  tx.at(ex).completedSetIndices.replace(next.indices);
  return next.added;
}

/** Moves an exercise one place earlier (-1) or later (1). */
export function moveExercise(tx: Edit, data: Data, id: string, step: -1 | 1) {
  const index = data.exercises.findIndex((ex) => ex.$id === id);
  if (index < 0) refuse("No such exercise.");
  const neighbor = data.exercises[index + step];
  if (!neighbor) return;
  tx.fields.exercises.move(id, step < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
}

export function resetProgress(tx: Edit, data: Data) {
  for (const ex of data.exercises) tx.at(ex).completedSetIndices.replace([]);
  return data.exercises.length;
}

export function setRestPreset(tx: Edit, seconds: string) {
  if (!(restPresets as readonly string[]).includes(seconds)) refuse(`Rest must be one of ${restPresets.join(", ")} seconds.`);
  tx.fields.restPreset.set(seconds as (typeof restPresets)[number]);
}
