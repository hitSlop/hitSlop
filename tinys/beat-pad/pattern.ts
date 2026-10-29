import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";

export type Beat = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

export const STEPS = 16;
export const BPM = { min: 60, max: 200 } as const;
export const SWING = { min: 0, max: 60 } as const;
export const kits = ["808", "lofi", "chip"] as const;
export type Kit = (typeof kits)[number];
export const kitLabel: Record<Kit, string> = { "808": "808", lofi: "Lo-fi", chip: "Chip" };

export const tracks = ["kick", "snare", "hat", "bass"] as const;
export type Track = (typeof tracks)[number];
export type Pattern = Record<Track, string>;

/** A step is one character: drums are "." or "x"; bass is "." or a note 1 to 5 of a minor pentatonic. */
export const alphabet: Record<Track, string> = { kick: ".x", snare: ".x", hat: ".x", bass: ".12345" };
export const trackLabel: Record<Track, string> = { kick: "Kick", snare: "Snare", hat: "Hat", bass: "Bass" };
export const bassNotes = ["C2", "Eb2", "F2", "G2", "Bb2"];

const blank = ".".repeat(STEPS);
const fit = (value: string) => value.padEnd(STEPS, ".").slice(0, STEPS);

export const patternOf = (data: Beat): Pattern => ({ kick: fit(data.kick), snare: fit(data.snare), hat: fit(data.hat), bass: fit(data.bass) });

export const empty = (): Pattern => ({ kick: blank, snare: blank, hat: blank, bass: blank });

export const cycle = (track: Track, value: string, index: number): string => {
  const steps = fit(value);
  const letters = alphabet[track];
  const next = letters[(letters.indexOf(steps[index] ?? ".") + 1) % letters.length]!;
  return steps.slice(0, index) + next + steps.slice(index + 1);
};

/** A fresh four-on-the-floor-ish pattern with a little chance in it. The caller supplies the random source. */
export function shuffle(random: () => number): Pattern {
  const make = (on: (step: number) => number) => Array.from({ length: STEPS }, (_, step) => (random() < on(step) ? "x" : ".")).join("");
  const bass = Array.from({ length: STEPS }, (_, step) => (step % 4 === 0 || random() < 0.18 ? String(1 + Math.floor(random() * 5)) : ".")).join("");
  return {
    kick: make((step) => (step % 4 === 0 ? 0.95 : step % 4 === 2 ? 0.25 : 0.08)),
    snare: make((step) => (step % 8 === 4 ? 0.95 : 0.08)),
    hat: make((step) => (step % 2 === 0 ? 0.9 : 0.35)),
    bass,
  };
}

/** The pad and its export read the grid from the same snapshot: two bars of eight steps per track. */
export function beatView(data: Beat) {
  const pattern = patternOf(data);
  const bars = [0, 1].map((half) => ({
    half,
    tracks: tracks.map((track) => ({
      track,
      name: trackLabel[track],
      cells: Array.from({ length: STEPS / 2 }, (_, i) => {
        const index = half * (STEPS / 2) + i;
        const value = pattern[track][index] ?? ".";
        const on = value !== ".";
        return {
          index,
          on,
          beat: index % 4 === 0,
          mark: on ? (track === "bass" ? value : "●") : "",
          label: `${trackLabel[track]} step ${index + 1}: ${on ? (track === "bass" ? `note ${value}` : "on") : "off"}`,
        };
      }),
    })),
  }));
  return { pattern, bars, kit: kitLabel[data.kit] };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
function checkSteps(track: Track, steps: string) {
  if (steps.length !== STEPS) refuse(`${trackLabel[track]} needs exactly ${STEPS} steps.`);
  const letters = alphabet[track];
  for (const ch of steps) {
    if (!letters.includes(ch)) refuse(track === "bass" ? "Bass steps are \".\" or a note 1 to 5." : `${trackLabel[track]} steps are "." or "x".`);
  }
}

export function setTrack(tx: Edit, track: Track, steps: string) {
  checkSteps(track, steps);
  tx.fields[track].set(steps);
}

/** Tap a step: drums toggle, bass cycles off → 1 … 5 → off. */
export function pressStep(tx: Edit, current: Beat, track: Track, index: number) {
  if (!Number.isInteger(index) || index < 0 || index >= STEPS) refuse(`Steps run from 1 to ${STEPS}.`);
  const next = cycle(track, current[track], index);
  tx.fields[track].set(next);
  return next[index];
}

/** Set one step (index from 0) to a value from the track's alphabet. */
export function setStep(tx: Edit, current: Beat, track: Track, index: number, value: string) {
  if (!Number.isInteger(index) || index < 0 || index >= STEPS) refuse(`Steps run from 1 to ${STEPS}.`);
  const steps = fit(current[track]);
  setTrack(tx, track, steps.slice(0, index) + value + steps.slice(index + 1));
}

export function setTempo(tx: Edit, bpm: number) {
  if (!Number.isInteger(bpm) || bpm < BPM.min || bpm > BPM.max) refuse(`Tempo runs from ${BPM.min} to ${BPM.max} BPM.`);
  tx.fields.bpm.set(bpm);
}

export function setSwing(tx: Edit, swing: number) {
  if (!Number.isInteger(swing) || swing < SWING.min || swing > SWING.max) refuse(`Swing runs from ${SWING.min} to ${SWING.max}%.`);
  tx.fields.swing.set(swing);
}

export function setKit(tx: Edit, kit: Kit) {
  if (!kits.includes(kit)) refuse(`Pick a kit: ${kits.join(", ")}.`);
  tx.fields.kit.set(kit);
}

export function writePattern(tx: Edit, pattern: Pattern) {
  for (const track of tracks) setTrack(tx, track, pattern[track]);
}

export const shufflePattern = (tx: Edit, random: () => number) => writePattern(tx, shuffle(random));
export const clearPattern = (tx: Edit) => writePattern(tx, empty());
