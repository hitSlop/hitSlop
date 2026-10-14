import { dateLabel } from "../_shared/format";
import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";

export type Pad = Value<typeof schema.descriptor>;
export type Note = Pad["notes"][number];
/** An attachment reference as `attachments.import` returns it and `clip` stores it. */
export type Clip = { readonly id: string; readonly byteLength: number; readonly name: string; readonly mimeType: string };
type Edit = Tx<typeof schema>;

/** One note records for at most five minutes: at 64 kbit/s that is about 2.4 MB, well
 * under the 10 MiB attachment limit. */
export const MAX_RECORDING_MS = 5 * 60 * 1000;
export const MAX_ATTACHMENT_BYTES = 10 * 1024 * 1024;
export const AUDIO_BITS_PER_SECOND = 64_000;

const formatNames: Record<string, string> = {
  "audio/mp4": "M4A", "audio/x-m4a": "M4A", "audio/m4a": "M4A", "audio/mpeg": "MP3", "audio/mp3": "MP3",
  "audio/wav": "WAV", "audio/x-wav": "WAV", "audio/wave": "WAV", "audio/vnd.wave": "WAV",
  "audio/flac": "FLAC", "audio/x-flac": "FLAC", "video/mp4": "MP4", "video/quicktime": "MOV",
  "audio/webm": "WebM", "video/webm": "WebM", "audio/ogg": "Ogg", "application/ogg": "Ogg",
};

export function baseType(mimeType: string) {
  return mimeType.split(";")[0]!.trim().toLowerCase();
}
function extension(name: string) {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}
/** The file extension a new recording of this media type is saved with. */
export function extensionFor(mimeType: string) {
  const type = baseType(mimeType);
  if (type === "audio/mp4" || type === "audio/x-m4a") return "m4a";
  if (type === "video/mp4") return "mp4";
  if (type.endsWith("/webm")) return "webm";
  if (type.endsWith("/ogg")) return "ogg";
  if (type === "audio/mpeg") return "mp3";
  return "audio";
}
export function formatName(clip: Clip) {
  return formatNames[baseType(clip.mimeType)] ?? (extension(clip.name).toUpperCase() || "Audio");
}

export function durationLabel(ms: number) {
  const seconds = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
export function sizeLabel(bytes: number) {
  return bytes < 1024 * 1024 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
export function whenLabel(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return { day: "Undated", time: "" };
  return { day: dateLabel(date, { weekday: "short", day: "numeric", month: "short" }), time: dateLabel(date, { hour: "numeric", minute: "2-digit" }) };
}
function words(text: string) {
  return text.trim() ? text.trim().split(/\s+/).length : 0;
}

/** The editor and the export derive ordering and totals from the same snapshot. */
export function padView(data: Pad) {
  const notes = data.notes
    .map((note, index) => ({ note, index }))
    .sort((a, b) => b.note.recordedAt.localeCompare(a.note.recordedAt) || b.index - a.index)
    .map(({ note }) => note);
  const recordings = notes.filter(note => note.clip);
  return {
    notes,
    count: notes.length,
    recordings: recordings.length,
    recordedMs: recordings.reduce((sum, note) => sum + note.durationMs, 0),
    transcribed: notes.filter(note => note.transcribed).length,
    words: notes.reduce((sum, note) => sum + words(note.transcript), 0),
  };
}

function checkInstant(iso: string) {
  if (!Number.isFinite(Date.parse(iso))) refuse("Give the time as an ISO 8601 instant.");
  return new Date(iso).toISOString();
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addTypedNote(tx: Edit, text: string, recordedAt: string) {
  return tx.fields.notes.insert({ recordedAt: checkInstant(recordedAt), transcript: text });
}
export function addRecording(tx: Edit, recording: { recordedAt: string; durationMs: number; clip: Clip }) {
  const { clip } = recording;
  if (!clip.id) refuse("A recording needs its attachment ID.");
  if (clip.byteLength > MAX_ATTACHMENT_BYTES) refuse("A recording can be at most 10 MiB.");
  return tx.fields.notes.insert({
    recordedAt: checkInstant(recording.recordedAt),
    durationMs: Math.max(0, Math.round(recording.durationMs)),
    clip: { id: clip.id, byteLength: clip.byteLength, name: clip.name, mimeType: clip.mimeType },
    transcript: "",
  });
}
export function setTranscript(tx: Edit, note: RowRef, text: string) {
  tx.fields.notes.item(note).transcript.set(text);
}
/** Saves an on-device transcription, replacing the note's text. */
export function saveTranscription(tx: Edit, note: RowRef, result: { text: string; locale: string; at: string }) {
  const row = tx.fields.notes.item(note);
  row.transcript.set(result.text);
  row.transcribed.$set({ locale: result.locale, at: checkInstant(result.at) });
}
export function removeNote(tx: Edit, note: RowRef) {
  tx.fields.notes.remove(note);
}
export function setLocale(tx: Edit, locale: string) {
  const value = locale.trim();
  if (!/^[A-Za-z0-9_-]{0,35}$/.test(value)) refuse("Use a locale identifier such as en-US, or leave it empty for the device's language.");
  tx.fields.locale.set(value);
}
export function renamePad(tx: Edit, title: string) {
  tx.fields.title.set(title.trim());
}

/** Languages offered in the picker; the document may hold any valid identifier. */
export const languages: { value: string; label: string }[] = [
  { value: "", label: "Device language" },
  { value: "en-US", label: "English (US)" },
  { value: "en-GB", label: "English (UK)" },
  { value: "en-AU", label: "English (Australia)" },
  { value: "fr-FR", label: "Français" },
  { value: "de-DE", label: "Deutsch" },
  { value: "es-ES", label: "Español" },
  { value: "it-IT", label: "Italiano" },
  { value: "pt-BR", label: "Português (Brasil)" },
  { value: "nl-NL", label: "Nederlands" },
  { value: "ja-JP", label: "日本語" },
  { value: "zh-CN", label: "中文 (普通话)" },
];
