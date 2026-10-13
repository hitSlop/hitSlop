import { dateLabel as formattedDate } from "../_shared/format";
import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import type { kinds } from "./schema";

export type Journal = typeof doc.current;
export type Entry = Journal["entries"][number];
export type Caregiver = Journal["caregivers"][number];
export type Kind = (typeof kinds)[number];
type Edit = Tx<typeof doc>;

/** One entry as the form or a command gives it; the edits below check and store it. */
export type EntryInput = {
  kind: Kind;
  timestamp: number;
  caregiverId: string;
  notes: string;
  feedingType: "bottle" | "nursing";
  unit: "mL" | "oz";
  side: "left" | "right" | "both";
  diaper: "wet" | "dirty" | "both";
  amount?: number;
  minutes?: number;
  endTimestamp?: number;
};

export function dayKey(value: number | Date): string {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "";
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function localInput(value: number): string {
  const date = new Date(value);
  return `${dayKey(date)}T${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}
export function timeLabel(value: number): string {
  return Number.isFinite(value)
    ? formattedDate(new Date(value), { hour: "numeric", minute: "2-digit" })
    : "Time unavailable";
}
export function dateLabel(day: string): string {
  const date = new Date(`${day}T12:00:00`);
  return Number.isFinite(date.getTime())
    ? formattedDate(date, { month: "long", day: "numeric", year: "numeric" })
    : "Choose a day";
}
export function duration(minutes: number): string {
  const total = Math.max(0, Math.round(minutes));
  const hours = Math.floor(total / 60);
  return hours ? `${hours}h${total % 60 ? ` ${total % 60}m` : ""}` : `${total} min`;
}
export function elapsed(timestamp: number, now: number): string {
  if (!Number.isFinite(timestamp) || timestamp > now) return "Time unavailable";
  const minutes = Math.floor((now - timestamp) / 60000);
  if (minutes < 1) return "Just now";
  if (minutes >= 1440) return `${Math.floor(minutes / 1440)}d ago`;
  return `${duration(minutes)} ago`;
}
export function entryTitle(entry: Entry): string {
  if (entry.kind === "feeding") {
    if (entry.feedingType === "nursing")
      return `Nursing · ${entry.side === "both" ? "both sides" : `${entry.side} side`}${entry.minutes ? ` · ${duration(entry.minutes)}` : ""}`;
    return `Bottle${entry.amount ? ` · ${entry.amount} ${entry.unit}` : ""}`;
  }
  if (entry.kind === "diaper")
    return `Diaper · ${entry.diaper === "both" ? "wet + dirty" : entry.diaper}`;
  if (entry.kind === "sleep")
    return entry.endTimestamp !== undefined && entry.endTimestamp > entry.timestamp
      ? `Sleep · ${duration((entry.endTimestamp - entry.timestamp) / 60000)}`
      : "Sleep · duration unavailable";
  return "A little memory";
}
export function babyAge(birthday: string, now: number): string {
  if (!birthday) return "One little day at a time";
  const born = new Date(`${birthday}T00:00:00`);
  const today = new Date(now);
  if (!Number.isFinite(born.getTime()) || born > today) return "Birthday unavailable";
  const days = Math.floor(
    (Date.UTC(today.getFullYear(), today.getMonth(), today.getDate()) -
      Date.UTC(born.getFullYear(), born.getMonth(), born.getDate())) /
      86400000,
  );
  if (days < 7) return `${days} ${days === 1 ? "day" : "days"} old`;
  if (days < 84) return `${Math.floor(days / 7)} weeks old`;
  const months =
    (today.getFullYear() - born.getFullYear()) * 12 +
    today.getMonth() -
    born.getMonth() -
    (today.getDate() < born.getDate() ? 1 : 0);
  return months < 24 ? `${months} months old` : `${Math.floor(months / 12)} years old`;
}

/** Newest first; ties keep a stable order by $id. */
function ordered(data: Journal): Entry[] {
  return [...data.entries].sort((a, b) => b.timestamp - a.timestamp || a.$id.localeCompare(b.$id));
}

export function careSummary(entries: readonly Entry[]) {
  return {
    feeds: entries.filter((entry) => entry.kind === "feeding").length,
    diapers: entries.filter((entry) => entry.kind === "diaper").length,
    sleepMinutes: entries.reduce(
      (total, entry) =>
        total + (entry.kind === "sleep" && entry.endTimestamp !== undefined && entry.endTimestamp > entry.timestamp
          ? (entry.endTimestamp - entry.timestamp) / 60000
          : 0),
      0,
    ),
  };
}

/** The editor derives its day, recent care and caregivers from one snapshot. */
export function journalView(data: Journal, day: string, now: number) {
  const all = ordered(data);
  const dayEntries = all.filter((entry) => dayKey(entry.timestamp) === day);
  return {
    dayEntries,
    summary: careSummary(dayEntries),
    lastFeed: all.find((entry) => entry.kind === "feeding" && entry.timestamp <= now),
    lastDiaper: all.find((entry) => entry.kind === "diaper" && entry.timestamp <= now),
    memory: all.find((entry) => entry.kind === "note" && entry.timestamp <= now),
    active: data.caregivers.filter((person) => !person.archived),
  };
}

/** The export derives its days, newest first, from the same snapshot. */
export function journalDays(data: Journal) {
  const groups = new Map<string, Entry[]>();
  for (const entry of ordered(data)) {
    const day = dayKey(entry.timestamp);
    groups.set(day, [...(groups.get(day) ?? []), entry]);
  }
  return Array.from(groups, ([day, entries]) => ({ day, entries, summary: careSummary(entries) }));
}

function checkEntry(data: Journal, input: EntryInput, now: number, keepCaregiver = ""): EntryInput {
  if (!Number.isFinite(input.timestamp) || input.timestamp > now) refuse("Choose a date and time that has already happened.");
  const person = data.caregivers.find((row) => row.$id === input.caregiverId);
  if (!person || (person.archived && person.$id !== keepCaregiver)) refuse("Choose who was there, or add a caregiver first.");
  const { kind, timestamp, endTimestamp } = input;
  const bottle = kind === "feeding" && input.feedingType === "bottle";
  const nursing = kind === "feeding" && input.feedingType === "nursing";
  if (kind === "sleep" && (endTimestamp === undefined || !Number.isFinite(endTimestamp) || endTimestamp <= timestamp || endTimestamp > now))
    refuse("Sleep must end after it starts, and no later than now.");
  if (bottle && input.amount !== undefined && (!Number.isFinite(input.amount) || input.amount <= 0))
    refuse("Enter an amount greater than zero, or leave it blank.");
  if (nursing && input.minutes !== undefined && (!Number.isFinite(input.minutes) || input.minutes <= 0))
    refuse("Enter a duration greater than zero, or leave it blank.");
  const notes = input.notes.trim();
  if (kind === "note" && !notes) refuse("Write a little something to remember.");
  return {
    kind,
    timestamp,
    caregiverId: input.caregiverId,
    notes,
    feedingType: input.feedingType,
    unit: input.unit,
    side: input.side,
    diaper: input.diaper,
    ...(bottle && input.amount !== undefined ? { amount: input.amount } : {}),
    ...(nursing && input.minutes !== undefined ? { minutes: input.minutes } : {}),
    ...(kind === "sleep" ? { endTimestamp } : {}),
  };
}

function remember(tx: Edit, fields: EntryInput) {
  tx.fields.lastCaregiverId.set(fields.caregiverId);
  if (fields.kind === "feeding" && fields.feedingType === "bottle") tx.fields.bottleUnit.set(fields.unit);
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addEntry(tx: Edit, data: Journal, input: EntryInput, now: number) {
  const fields = checkEntry(data, input, now);
  remember(tx, fields);
  return tx.fields.entries.insert(fields);
}
export function updateEntry(tx: Edit, data: Journal, id: string, input: EntryInput, now: number) {
  const existing = data.entries.find((entry) => entry.$id === id);
  if (!existing) refuse("This entry is no longer in the journal. Close this form and add a new entry.");
  if (input.kind !== existing.kind) refuse("An entry keeps its kind. Add a new entry instead.");
  const fields = checkEntry(data, input, now, existing.caregiverId);
  const handle = tx.at(existing);
  handle.timestamp.set(fields.timestamp);
  handle.caregiverId.set(fields.caregiverId);
  handle.notes.set(fields.notes);
  handle.feedingType.set(fields.feedingType);
  handle.unit.set(fields.unit);
  handle.side.set(fields.side);
  handle.diaper.set(fields.diaper);
  if (fields.amount === undefined) handle.amount.clear(); else handle.amount.set(fields.amount);
  if (fields.minutes === undefined) handle.minutes.clear(); else handle.minutes.set(fields.minutes);
  if (fields.endTimestamp === undefined) handle.endTimestamp.clear(); else handle.endTimestamp.set(fields.endTimestamp);
  remember(tx, fields);
}
export function removeEntry(tx: Edit, data: Journal, id: string) {
  if (!data.entries.some((entry) => entry.$id === id)) refuse("That entry is already gone.");
  tx.fields.entries.remove(id);
}
export function addCaregiver(tx: Edit, data: Journal, name: string) {
  const trimmed = name.trim();
  if (!trimmed) refuse("Enter a caregiver's name.");
  if (data.caregivers.some((person) => person.name.trim().toLocaleLowerCase() === trimmed.toLocaleLowerCase()))
    refuse("That name is already here. You can restore an archived caregiver below.");
  return tx.fields.caregivers.insert({ name: trimmed });
}
export function setCaregiverArchived(tx: Edit, data: Journal, id: string, archived: boolean) {
  const person = data.caregivers.find((row) => row.$id === id);
  if (!person) refuse("That caregiver is no longer here.");
  if (person.archived === archived) return;
  if (archived && data.caregivers.filter((row) => !row.archived).length <= 1)
    refuse("Keep at least one caregiver active.");
  tx.at(person).archived.set(archived);
}
