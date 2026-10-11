import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";

export type Sheet = typeof doc.current;
type Edit = Tx<typeof doc>;
export type Session = Sheet["log"][number];

export const TEMPO_MIN = 20;
export const TEMPO_MAX = 300;
export const STEP_MAX = 40;
export const MINUTES_MAX = 600;
export const ABC_MAX = 20_000;

// ---------------------------------------------------------------- dates (local days)

const DAY = /^(\d{4})-(\d{2})-(\d{2})$/;

/** A Date's local calendar day as YYYY-MM-DD. */
export function localDay(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function isDay(value: string): boolean {
  const match = DAY.exec(value);
  if (!match) return false;
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return localDay(date) === value;
}

/** The day `n` days after `day` (negative goes back). */
export function addDays(day: string, n: number): string {
  const [y, m, d] = day.split("-").map(Number);
  return localDay(new Date(y, m - 1, d + n));
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS = ["S", "M", "T", "W", "T", "F", "S"];
export function shortDay(day: string): string {
  const [, m, d] = day.split("-").map(Number);
  return `${MONTHS[m - 1]} ${d}`;
}
function weekday(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  return WEEKDAYS[new Date(y, m - 1, d).getDay()];
}

// ---------------------------------------------------------------- ABC header

export type TuneHeader = { title: string; composer: string; meter: string; key: string };

/** Reads the header fields (before the first K: line) of the first tune. */
export function tuneHeader(abc: string): TuneHeader {
  const header: TuneHeader = { title: "", composer: "", meter: "", key: "" };
  for (const raw of abc.split(/\r?\n/)) {
    const line = raw.trim();
    const field = /^([A-Za-z]):\s*(.*)$/.exec(line);
    if (!field) continue;
    const [, name, value] = field;
    if (name === "T" && !header.title) header.title = value.trim();
    else if (name === "C" && !header.composer) header.composer = value.trim();
    else if (name === "M" && !header.meter) header.meter = value.trim();
    else if (name === "K") { header.key = value.trim(); break; }
  }
  return header;
}

/**
 * The text handed to the engraver: header title and composer lines become comments of
 * the same length, so the page shows them in its own type and character offsets still
 * match the stored ABC.
 */
export function engraveSource(abc: string): string {
  let inHeader = true;
  return abc.split("\n").map(line => {
    if (!inHeader) return line;
    if (/^\s*K:/.test(line)) inHeader = false;
    return /^\s*[TC]:/.test(line) ? line.replace(/^(\s*)[TC]:/, "$1% ") : line;
  }).join("\n");
}

const keyNames: Record<string, string> = { m: "minor", min: "minor", maj: "major", "": "major", dor: "Dorian", mix: "Mixolydian", lyd: "Lydian", phr: "Phrygian", loc: "Locrian", aeo: "minor", ion: "major" };
/** "G" → "G major", "Ador" → "A Dorian"; unknown forms are returned as written. */
export function keyLabel(key: string): string {
  const match = /^([A-G])([#b]?)\s*([A-Za-z]*)/.exec(key);
  if (!match) return key;
  const mode = keyNames[match[3].toLowerCase().slice(0, 3)] ?? keyNames[match[3].toLowerCase()];
  if (mode === undefined) return key;
  const accidental = match[2] === "#" ? "♯" : match[2] === "b" ? "♭" : "";
  return `${match[1]}${accidental} ${mode}`;
}

// ---------------------------------------------------------------- view

/** Every tempo on the ladder from start to goal, the goal always last. */
export function ladderRungs(ladder: Sheet["ladder"]): number[] {
  const rungs: number[] = [];
  for (let bpm = ladder.start; bpm < ladder.goal && rungs.length < 200; bpm += ladder.step) rungs.push(bpm);
  rungs.push(ladder.goal);
  return rungs;
}

function streakEnding(days: Set<string>, last: string): number {
  let count = 0;
  for (let day = last; days.has(day); day = addDays(day, -1)) count++;
  return count;
}

/** Everything the editor and the export show, derived from one snapshot and today's date. */
export function practiceView(data: Sheet, today: string) {
  const header = tuneHeader(data.abc);
  const { ladder, tempo } = data;
  const rungs = ladderRungs(ladder);
  const span = Math.max(1, ladder.goal - ladder.start);
  const progress = Math.min(1, Math.max(0, (tempo - ladder.start) / span));
  const atGoal = tempo >= ladder.goal;

  const minutesByDay = new Map<string, number>();
  for (const session of data.log) minutesByDay.set(session.at, (minutesByDay.get(session.at) ?? 0) + session.minutes);
  const days = new Set(minutesByDay.keys());
  // A streak stays alive through today until today is over.
  const streak = days.has(today) ? streakEnding(days, today) : streakEnding(days, addDays(today, -1));
  // The longest run: walk forwards from each run's first day.
  let best = 0;
  for (const day of days) {
    if (days.has(addDays(day, -1))) continue;
    let length = 0;
    for (let d = day; days.has(d); d = addDays(d, 1)) length++;
    best = Math.max(best, length);
  }
  const fortnight = Array.from({ length: 14 }, (_, i) => {
    const day = addDays(today, i - 13);
    return { day, weekday: weekday(day), minutes: minutesByDay.get(day) ?? 0, today: day === today };
  });
  const sessions = data.log.map((session, index) => ({ session, index }))
    .sort((a, b) => (a.session.at === b.session.at ? b.index - a.index : a.session.at < b.session.at ? 1 : -1))
    .map(entry => entry.session);
  const totalMinutes = data.log.reduce((sum, session) => sum + session.minutes, 0);
  const bestTempo = data.log.reduce((max, session) => Math.max(max, session.tempo), 0);

  return {
    header, tempo, ladder, rungs, progress, atGoal,
    nextTempo: Math.min(ladder.goal, tempo + ladder.step),
    streak, best, fortnight, sessions, totalMinutes, bestTempo,
    todayMinutes: minutesByDay.get(today) ?? 0,
    lastSession: sessions[0] as Session | undefined,
  };
}

// ---------------------------------------------------------------- edits (written once)

function checkTempo(bpm: number, label = "Tempo") {
  if (!Number.isInteger(bpm) || bpm < TEMPO_MIN || bpm > TEMPO_MAX)
    refuse(`${label} must be a whole number from ${TEMPO_MIN} to ${TEMPO_MAX} bpm.`);
}

export function setTempo(tx: Edit, bpm: number) {
  checkTempo(bpm);
  tx.fields.tempo.set(bpm);
  return bpm;
}

/** A clean run climbs one rung, never past the goal. Returns the new tempo. */
export function cleanRun(tx: Edit, current: Sheet) {
  const { tempo, ladder } = current;
  if (tempo >= ladder.goal) refuse(`You're already at the goal of ${ladder.goal} bpm.`);
  const next = Math.min(ladder.goal, tempo + ladder.step);
  tx.fields.tempo.set(next);
  return next;
}

/** Back to the bottom rung. Returns the new tempo. */
export function restartLadder(tx: Edit, current: Sheet) {
  tx.fields.tempo.set(current.ladder.start);
  return current.ladder.start;
}

export function setLadder(tx: Edit, current: Sheet, change: { start?: number; goal?: number; step?: number }) {
  const next = { ...current.ladder, ...Object.fromEntries(Object.entries(change).filter(([, v]) => v !== undefined)) };
  checkTempo(next.start, "The starting tempo");
  checkTempo(next.goal, "The goal");
  if (next.goal <= next.start) refuse("The goal must be faster than the starting tempo.");
  if (!Number.isInteger(next.step) || next.step < 1 || next.step > STEP_MAX)
    refuse(`The step must be a whole number from 1 to ${STEP_MAX} bpm.`);
  const row = tx.fields.ladder;
  if (next.start !== current.ladder.start) row.start.set(next.start);
  if (next.goal !== current.ladder.goal) row.goal.set(next.goal);
  if (next.step !== current.ladder.step) row.step.set(next.step);
  return next;
}

/** Logs a session at the current tempo and returns its row ID. */
export function logPractice(tx: Edit, current: Sheet, at: string, minutes: number) {
  if (!isDay(at)) refuse("Give the day as YYYY-MM-DD.");
  if (!Number.isInteger(minutes) || minutes < 1 || minutes > MINUTES_MAX)
    refuse(`Log between 1 and ${MINUTES_MAX} whole minutes.`);
  return tx.fields.log.insert({ at, minutes, tempo: current.tempo });
}

export function removeSession(tx: Edit, session: RowRef) {
  tx.fields.log.remove(session);
}

/** Replaces the whole piece. The page edits the text in place instead. */
export function replaceAbc(tx: Edit, abc: string) {
  const text = abc.replace(/\r\n?/g, "\n").trim();
  if (!text) refuse("Paste some ABC notation.");
  if (text.length > ABC_MAX) refuse(`Keep the piece under ${ABC_MAX} characters.`);
  if (!/^\s*K:/m.test(text)) refuse("ABC needs a K: (key) line before the notes.");
  tx.fields.abc.set(text);
  return tuneHeader(text).title || "Untitled";
}
