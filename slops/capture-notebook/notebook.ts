import { refuse, type Tx } from "hitslop";
import type doc from "./schema";

export type Notebook = typeof doc.current;
export type Entry = Notebook["entries"][number];
type Edit = Tx<typeof doc>;
/** An attachment reference as `attachments.import` returns it. */
export type FileRef = { id: string; name: string; mimeType: string; byteLength: number };

/** One attachment holds at most 10 MiB; a document at most 256 files and 100 MiB. */
export const MAX_FILE_BYTES = 10 * 1024 * 1024;
export const MAX_FILES = 256;
export const MAX_TOTAL_BYTES = 100 * 1024 * 1024;
/** Five minutes at 64 kbit/s is about 2.4 MB, well under the attachment limit. */
export const MAX_RECORDING_SECONDS = 300;
export const AUDIO_BITS_PER_SECOND = 64_000;
/** Photos are scaled so the longest edge is at most this many pixels. */
export const PHOTO_LONGEST_EDGE = 1600;
const MAX_CAPTION = 2000;

/** The editor and its export derive order, counts and totals from the same snapshot. */
export function notebookView(data: Notebook) {
  const entries = [...data.entries].sort((a, b) => b.at.localeCompare(a.at) || a.$id.localeCompare(b.$id));
  const photos = entries.filter(entry => entry.kind === "photo");
  const recordings = entries.filter(entry => entry.kind === "audio");
  const notes = entries.filter(entry => entry.kind === "note");
  const files = entries.filter(entry => entry.file);
  return {
    entries, photos, recordings, notes,
    fileCount: files.length,
    totalBytes: files.reduce((sum, entry) => sum + (entry.file?.byteLength ?? 0), 0),
    totalSeconds: recordings.reduce((sum, entry) => sum + (entry.seconds ?? 0), 0),
  };
}

/** Why a new file cannot be kept, or "" when it fits the document's limits. */
export function roomFor(data: Notebook, bytes: number): string {
  const { fileCount, totalBytes } = notebookView(data);
  if (bytes > MAX_FILE_BYTES) return `This is ${formatBytes(bytes)}; one file can be at most 10 MB.`;
  if (fileCount >= MAX_FILES) return "This notebook holds 256 files already. Remove one to keep another.";
  if (totalBytes + bytes > MAX_TOTAL_BYTES) return "This notebook is full (100 MB). Remove a photo or recording to make room.";
  return "";
}

function fileRef(ref: FileRef) {
  return { id: ref.id, name: ref.name, mimeType: ref.mimeType, byteLength: ref.byteLength };
}
function checkTime(at: string) {
  if (!Number.isFinite(Date.parse(at))) refuse("That time is not a valid date.");
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function keepPhoto(tx: Edit, ref: FileRef, at: string, width: number, height: number) {
  checkTime(at);
  return tx.fields.entries.insert({
    kind: "photo", at, caption: "", file: fileRef(ref),
    width: Math.max(1, Math.round(width)), height: Math.max(1, Math.round(height)),
  });
}
export function keepRecording(tx: Edit, ref: FileRef, at: string, seconds: number) {
  checkTime(at);
  return tx.fields.entries.insert({
    kind: "audio", at, caption: "", file: fileRef(ref),
    seconds: Math.max(0, Math.round(seconds * 10) / 10),
  });
}
export function addNote(tx: Edit, text: string, at: string) {
  const trimmed = text.trim();
  if (!trimmed) refuse("Write something to keep.");
  if ([...trimmed].length > MAX_CAPTION) refuse("Keep a note under 2,000 characters.");
  checkTime(at);
  return tx.fields.entries.insert({ kind: "note", at, caption: trimmed });
}
export function setCaption(tx: Edit, data: Notebook, id: string, caption: string) {
  const entry = data.entries.find(row => row.$id === id);
  if (!entry) refuse("That entry is no longer in the notebook.");
  if ([...caption].length > MAX_CAPTION) refuse("Keep a caption under 2,000 characters.");
  if (entry.kind === "note" && !caption.trim()) refuse("A note needs some text. Remove it instead.");
  tx.at(entry).caption.set(caption.trim());
}
/** Removing the entry releases its file: unreferenced attachments are dropped when the document closes. */
export function removeEntry(tx: Edit, data: Notebook, id: string) {
  if (!data.entries.some(row => row.$id === id)) refuse("That entry is already gone.");
  tx.fields.entries.remove(id);
}
export function rename(tx: Edit, title: string) {
  const trimmed = title.trim();
  if (!trimmed) refuse("Give the notebook a name.");
  tx.fields.title.set(trimmed);
}

// Labels, shared by the editor and the export.
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
export function clock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}
export function stamp(at: string): string {
  const date = new Date(at);
  if (!Number.isFinite(date.getTime())) return "Time unknown";
  const day = date.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" });
  const time = date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return `${day} · ${time}`;
}
export function formatLabel(entry: Entry): string {
  const type = entry.file?.mimeType ?? "";
  const name = entry.file?.name ?? "";
  const ext = /\.([a-z0-9]+)$/i.exec(name)?.[1]?.toUpperCase();
  return ext ?? (type.split("/")[1]?.toUpperCase() || "FILE");
}
/** A descriptive file name such as `photo-2026-10-04-0840.jpg`, in local time. */
export function fileName(kind: "photo" | "recording", now: Date, ext: string): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  const day = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  return `${kind}-${day}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}.${ext}`;
}
export function summary(view: ReturnType<typeof notebookView>): string {
  const parts = [
    `${view.photos.length} ${view.photos.length === 1 ? "photo" : "photos"}`,
    `${view.recordings.length} ${view.recordings.length === 1 ? "recording" : "recordings"}`,
    `${view.notes.length} ${view.notes.length === 1 ? "note" : "notes"}`,
  ];
  return parts.join(" · ");
}
