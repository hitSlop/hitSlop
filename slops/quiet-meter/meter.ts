import { dateLabel } from "../_shared/format";
import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { KEPT_SESSIONS, LEVEL, SUSTAIN } from "./schema";

export type Data = Value<typeof schema.descriptor>;
export type Session = Data["sessions"][number];
type Edit = Tx<typeof schema>;
export interface SessionInput { date: string; seconds: number; loudSeconds: number; episodes: number; peak: number }
export type Zone = "quiet" | "warn" | "loud";

export const ZONE_LABEL: Record<Zone, string> = { quiet: "Quiet", warn: "Getting loud", loud: "Too loud" };
const FLOOR_DB = -80;

/** Level 0-100 from a root-mean-square amplitude (0-1). Relative to the microphone, not calibrated sound pressure. */
export function levelFromRms(rms: number) {
  if (!(rms > 0)) return 0;
  const db = 20 * Math.log10(rms);
  return Math.min(100, Math.max(0, ((db - FLOOR_DB) / -FLOOR_DB) * 100));
}

/** Fast attack, slow release, so a clap registers and a gap does not flicker. */
export function smoothLevel(previous: number, next: number, dt: number) {
  const tau = next > previous ? 0.08 : 0.7;
  return previous + (next - previous) * (1 - Math.exp(-dt / tau));
}

export function zoneOf(level: number, amber: number, red: number): Zone {
  return level >= red ? "loud" : level >= amber ? "warn" : "quiet";
}

export interface Tally { seconds: number; loudSeconds: number; episodes: number; peak: number; run: number; counted: boolean }
export const emptyTally = (): Tally => ({ seconds: 0, loudSeconds: 0, episodes: 0, peak: 0, run: 0, counted: false });

/** Advance the transient session tally by `dt` seconds. Red must last `sustain` seconds to count as too loud. */
export function advanceTally(tally: Tally, level: number, zone: Zone, dt: number, sustain: number): Tally {
  const next = { ...tally, seconds: tally.seconds + dt, peak: Math.max(tally.peak, level) };
  if (zone !== "loud") return { ...next, run: 0, counted: false };
  next.run += dt;
  if (next.run < sustain) return next;
  if (!next.counted) return { ...next, counted: true, episodes: next.episodes + 1, loudSeconds: next.loudSeconds + next.run };
  return { ...next, loudSeconds: next.loudSeconds + dt };
}

export function clock(totalSeconds: number) {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
  const two = (n: number) => String(n).padStart(2, "0");
  return h ? `${h}:${two(m)}:${two(sec)}` : `${m}:${two(sec)}`;
}

export function localDay(date: Date) {
  const two = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
}

export function dayLabel(day: string) {
  const [y, m, d] = day.split("-").map(Number);
  if (!y || !m || !d) return day;
  return dateLabel(new Date(y, m - 1, d), { weekday: "short", month: "short", day: "numeric" });
}

/** The editor and its captures read the same derived values. */
export function meterView(data: Data) {
  const sessions = data.sessions;
  const last = sessions.at(-1);
  return {
    sessions,
    last,
    recent: sessions.slice(-3).reverse(),
    lastZone: last ? zoneOf(last.peak, data.amber, data.red) : undefined,
    room: data.room.trim() || "This room",
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setThresholds(tx: Edit, amber: number, red: number) {
  if (!Number.isInteger(amber) || !Number.isInteger(red)) refuse("Thresholds are whole numbers.");
  if (amber < LEVEL.min || red > LEVEL.max) refuse(`Thresholds run from ${LEVEL.min} to ${LEVEL.max}.`);
  if (red < amber + 5) refuse("Red must be at least 5 above amber.");
  tx.fields.amber.set(amber);
  tx.fields.red.set(red);
}
export function setSustain(tx: Edit, seconds: number) {
  if (!Number.isInteger(seconds) || seconds < SUSTAIN.min || seconds > SUSTAIN.max) {
    refuse(`Seconds must be a whole number from ${SUSTAIN.min} to ${SUSTAIN.max}.`);
  }
  tx.fields.sustain.set(seconds);
}
export function saveSession(tx: Edit, current: Data, session: SessionInput) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(session.date)) refuse("Date must be YYYY-MM-DD.");
  if (session.seconds < 1) refuse("A session needs at least one second of listening.");
  if (session.loudSeconds > session.seconds) refuse("Too-loud time cannot exceed listening time.");
  const id = tx.fields.sessions.insert(session);
  const extra = current.sessions.length + 1 - KEPT_SESSIONS;
  for (const old of current.sessions.slice(0, Math.max(0, extra))) tx.fields.sessions.remove(old.$id);
  return id;
}
export function clearSessions(tx: Edit, current: Data) {
  for (const row of current.sessions) tx.fields.sessions.remove(row.$id);
  return current.sessions.length;
}
