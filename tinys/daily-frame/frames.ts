import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { maxFrames } from "./schema";

export type Data = Value<typeof schema.descriptor>;
export type Frame = Data["frames"][number];
type Edit = Tx<typeof schema>;

/** A frame as the views draw it: a saved photo, or a drawn sample before the first photo. */
export type Shown = {
  key: string;
  date: string;
  note: string;
  /** The attachment ID, or null for a drawn sample. */
  photo: string | null;
  /** How far the sample seedling has grown, 0 to 1 (samples only). */
  growth: number;
  frame: Frame | null;
};

// ---- Local days -------------------------------------------------------------------

const dayPattern = /^(\d{4})-(\d{2})-(\d{2})$/;
const pad = (n: number) => String(n).padStart(2, "0");

/** The local calendar day of `date` as YYYY-MM-DD. */
export function dayKey(date: Date) {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}
/** True for a real calendar day written YYYY-MM-DD. */
export function isDay(value: string) {
  const match = dayPattern.exec(value);
  if (!match) return false;
  const [y, m, d] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const date = new Date(Date.UTC(y, m - 1, d));
  return date.getUTCFullYear() === y && date.getUTCMonth() === m - 1 && date.getUTCDate() === d;
}
function utc(day: string) {
  const [y, m, d] = day.split("-").map(Number) as [number, number, number];
  return Date.UTC(y, m - 1, d);
}
function fromUtc(ms: number) {
  const date = new Date(ms);
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}
export function addDays(day: string, days: number) {
  return fromUtc(utc(day) + days * 86_400_000);
}
export function daysBetween(from: string, to: string) {
  return Math.round((utc(to) - utc(from)) / 86_400_000);
}
/** The same day of the month one month earlier, clamped to that month's length (Mar 31 -> Feb 28). */
export function monthEarlier(day: string) {
  const [y, m, d] = day.split("-").map(Number) as [number, number, number];
  const last = new Date(Date.UTC(y, m - 1, 0)).getUTCDate();
  return fromUtc(Date.UTC(y, m - 2, Math.min(d, last)));
}
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
/** "Fri, Oct 10" */
export function dayLabel(day: string) {
  const date = new Date(utc(day));
  return `${weekdays[date.getUTCDay()]}, ${months[date.getUTCMonth()]} ${date.getUTCDate()}`;
}
/** "Oct 10" */
export function shortLabel(day: string) {
  const date = new Date(utc(day));
  return `${months[date.getUTCMonth()]} ${date.getUTCDate()}`;
}
/** The film-camera date imprint: '26 10 10 */
export function stamp(day: string) {
  return `'${day.slice(2, 4)} ${day.slice(5, 7)} ${day.slice(8, 10)}`;
}
export function sizeLabel(bytes: number) {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

// ---- The stack --------------------------------------------------------------------

/** A drawn seedling shown until the first photo is kept. Dated relative to `today`; never stored. */
const sampleDays: [offset: number, note: string][] = [
  [-33, "Planted three seeds"], [-31, ""], [-28, "First green!"], [-26, ""], [-24, ""], [-21, "Two leaves"],
  [-19, ""], [-16, "Turned it toward the light"], [-13, ""], [-11, ""], [-8, "Smells like pasta"], [-6, ""],
  [-3, ""], [-1, "Needs a bigger pot"],
];
export function sampleStack(today: string): Shown[] {
  return sampleDays.map(([offset, note], index) => ({
    key: `sample-${index}`, date: addDays(today, offset), note, photo: null,
    growth: index / (sampleDays.length - 1), frame: null,
  }));
}

/** The editor and its captures derive the stack, the readout and today's slot from the same snapshot. */
export function frameView(data: Data, today: string) {
  const frames = [...data.frames].sort((a, b) => (a.date < b.date ? -1 : a.date > b.date ? 1 : 0));
  const sample = frames.length === 0;
  const stack: Shown[] = sample ? sampleStack(today) : frames.map(frame => ({
    key: frame.$id, date: frame.date, note: frame.note, photo: frame.attachment.id, growth: 1, frame,
  }));
  const byDate = new Map(stack.map(shown => [shown.date, shown]));
  let streak = 0;
  for (let day = byDate.has(today) ? today : addDays(today, -1); byDate.has(day); day = addDays(day, -1)) streak++;
  const earlier = stack.filter(shown => shown.date < today);
  return {
    frames, sample, stack, byDate, streak,
    todayFrame: byDate.get(today) ?? null,
    /** The newest saved frame before today: the ghost to line up today's photo with. */
    previous: sample ? null : earlier.at(-1) ?? null,
    latest: stack.at(-1) ?? null,
    first: stack[0] ?? null,
    bytes: frames.reduce((total, frame) => total + frame.attachment.byteLength, 0),
    full: frames.length >= maxFrames,
  };
}
export type FrameView = ReturnType<typeof frameView>;

/** The frame on `day`, or the nearest one within `within` days (earlier wins a tie). */
export function nearFrame(view: FrameView, day: string, within = 3): Shown | null {
  for (let distance = 0; distance <= within; distance++) {
    const found = view.byDate.get(addDays(day, -distance)) ?? view.byDate.get(addDays(day, distance));
    if (found) return found;
  }
  return null;
}

// ---- Edits (written once: the page runs them in doc.change, commands with their tx) ----

type PhotoRef = { id: string; mimeType: string; byteLength: number };

function frameOn(current: Data, date: string) {
  if (!isDay(date)) refuse("Write the day as YYYY-MM-DD.");
  const frame = current.frames.find(row => row.date === date);
  if (!frame) refuse(`There is no frame for ${date}.`);
  return frame;
}

/** Keeps `photo` as the frame for `date`. A retake replaces that day's photo and keeps its
 * note; the old photo is no longer referenced, so the host removes it when the file closes. */
export function keepFrame(tx: Edit, current: Data, date: string, photo: PhotoRef) {
  if (!isDay(date)) refuse("Write the day as YYYY-MM-DD.");
  if (!photo.mimeType.startsWith("image/")) refuse("A frame must be a photo.");
  const attachment = { id: photo.id, mimeType: photo.mimeType, byteLength: photo.byteLength };
  const existing = current.frames.find(row => row.date === date);
  if (existing) {
    const row = tx.at(existing).attachment;
    row.id.set(attachment.id);
    row.mimeType.set(attachment.mimeType);
    row.byteLength.set(attachment.byteLength);
    return existing.$id;
  }
  if (current.frames.length >= maxFrames) refuse(`This file already holds ${maxFrames} frames, the most one document can keep. Start a new Daily Frame to continue.`);
  const later = current.frames.find(row => row.date > date);
  return tx.fields.frames.insert({ date, attachment, note: "" }, later ? { before: later } : undefined).id;
}

export function setNote(tx: Edit, current: Data, date: string, note: string) {
  const frame = frameOn(current, date);
  tx.at(frame).note.set(note.trim());
  return frame.$id;
}

export function removeFrame(tx: Edit, current: Data, date: string) {
  tx.fields.frames.remove(frameOn(current, date));
}

/** Moves a frame to another day, for a photo filed under the wrong date. */
export function redateFrame(tx: Edit, current: Data, date: string, to: string) {
  const frame = frameOn(current, date);
  if (!isDay(to)) refuse("Write the new day as YYYY-MM-DD.");
  if (to === date) return frame.$id;
  if (current.frames.some(row => row.date === to)) refuse(`${to} already has a frame. Remove it first.`);
  tx.at(frame).date.set(to);
  // Keep the stored list in date order too, for readers of `tiny get`.
  const others = current.frames.filter(row => row.$id !== frame.$id);
  const later = others.find(row => row.date > to);
  const last = others.at(-1);
  if (later) tx.fields.frames.move(frame, { before: later });
  else if (last) tx.fields.frames.move(frame, { after: last });
  return frame.$id;
}

export function rename(tx: Edit, title: string) {
  if (!title.trim()) refuse("Enter a title.");
  tx.fields.title.set(title.trim());
}
