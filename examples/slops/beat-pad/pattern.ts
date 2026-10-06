export const STEPS = 16;
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

export const empty = (): Pattern => ({ kick: ".".repeat(STEPS), snare: ".".repeat(STEPS), hat: ".".repeat(STEPS), bass: ".".repeat(STEPS) });

export const cycle = (track: Track, value: string, index: number): string => {
  const letters = alphabet[track];
  const next = letters[(letters.indexOf(value[index] ?? ".") + 1) % letters.length]!;
  return value.slice(0, index) + next + value.slice(index + 1);
};

/** A fresh four-on-the-floor-ish pattern with a little chance in it. */
export function shuffle(random: () => number = Math.random): Pattern {
  const make = (on: (step: number) => number) => Array.from({ length: STEPS }, (_, step) => (random() < on(step) ? "x" : ".")).join("");
  const bass = Array.from({ length: STEPS }, (_, step) => (step % 4 === 0 || random() < 0.18 ? String(1 + Math.floor(random() * 5)) : ".")).join("");
  return {
    kick: make((step) => (step % 4 === 0 ? 0.95 : step % 4 === 2 ? 0.25 : 0.08)),
    snare: make((step) => (step % 8 === 4 ? 0.95 : 0.08)),
    hat: make((step) => (step % 2 === 0 ? 0.9 : 0.35)),
    bass,
  };
}
