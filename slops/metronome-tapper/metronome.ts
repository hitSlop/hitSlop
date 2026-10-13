import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { signatures } from "./schema";

export type Edit = Tx<typeof schema>;
export type Data = Value<typeof schema.descriptor>;
export type TimeSignature = (typeof signatures)[number];

export const MIN_BPM = 40;
export const MAX_BPM = 240;
export const MAX_PRESETS = 6;

export function clampBpm(value: number): number {
  return Math.min(MAX_BPM, Math.max(MIN_BPM, Math.round(value)));
}

export function isSignature(value: string): value is TimeSignature {
  return (signatures as readonly string[]).includes(value);
}

export function tempoName(bpm: number): string {
  if (bpm < 60) return "Largo";
  if (bpm < 76) return "Adagio";
  if (bpm < 108) return "Andante";
  if (bpm < 120) return "Moderato";
  if (bpm < 156) return "Allegro";
  if (bpm < 200) return "Vivace";
  return "Presto";
}

/** The editor, its icon and its captures read the same derived values. */
export function metronomeView(data: Data) {
  return {
    beatsPerBar: Number.parseInt(data.signature.split("/")[0] ?? "4", 10),
    tempoName: tempoName(data.bpm),
    weight: `${((data.bpm - MIN_BPM) / (MAX_BPM - MIN_BPM)) * 58 + 18}%`,
  };
}

/** Tempo from tap times in milliseconds, or null until two taps exist. */
export function tapTempo(taps: readonly number[]): number | null {
  if (taps.length < 2) return null;
  const span = taps[taps.length - 1]! - taps[0]!;
  if (span <= 0) return null;
  return clampBpm(60000 / (span / (taps.length - 1)));
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setTempo(tx: Edit, bpm: number) {
  if (!Number.isFinite(bpm) || bpm < MIN_BPM || bpm > MAX_BPM) refuse(`Tempo must be between ${MIN_BPM} and ${MAX_BPM} BPM.`);
  tx.fields.bpm.set(Math.round(bpm));
}
export function setSignature(tx: Edit, signature: string) {
  if (!isSignature(signature)) refuse(`Time signature must be one of ${signatures.join(", ")}.`);
  tx.fields.signature.set(signature);
}
export function setVolume(tx: Edit, volume: number) {
  if (!Number.isFinite(volume) || volume < 0 || volume > 1) refuse("Volume must be between 0 and 1.");
  tx.fields.volume.set(volume);
}
export function setMuted(tx: Edit, muted: boolean) {
  tx.fields.muted.set(muted);
}
/** Saves the current tempo as a preset, keeping presets sorted and at most six. */
export function savePreset(tx: Edit, current: Data) {
  const bpm = current.bpm;
  const presets = current.presets;
  if (presets.includes(bpm)) refuse(`${bpm} BPM is already a preset.`);
  const merged = [...presets, bpm].sort((a, b) => a - b).slice(0, MAX_PRESETS);
  const count = Math.min(presets.length, merged.length);
  for (let index = 0; index < count; index += 1) {
    if (presets[index] !== merged[index]) tx.fields.presets.set(index, merged[index]!);
  }
  for (let index = presets.length; index < merged.length; index += 1) tx.fields.presets.insert(merged[index]!, index);
  if (presets.length > merged.length) tx.fields.presets.remove(merged.length, presets.length - merged.length);
}
export function removePreset(tx: Edit, current: Data, bpm: number) {
  const index = current.presets.indexOf(bpm);
  if (index < 0) refuse(`${bpm} BPM is not a preset.`);
  if (current.presets.length <= 1) refuse("Keep at least one preset.");
  tx.fields.presets.remove(index);
}
