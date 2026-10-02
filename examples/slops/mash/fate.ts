import { slots, type Option, type Slot } from "./schema";

export const slotLabel: Record<Slot, string> = {
  home: "Home", spouse: "Marry", kids: "Kids", car: "Drive", job: "Job", city: "City", pet: "Pet",
};

export const maxOptions = 6;

export type Fate = {
  /** Option ids in the order the pen struck them out. */
  struck: string[];
  /** The surviving option id for each slot that has any. */
  picks: Partial<Record<Slot, string>>;
};

// The classic count: walk the options in a ring, strike every nth one, skipping a
// slot once only one of its options is left. Empty options are not in the game.
export function play(options: readonly Option[], loops: number): Fate {
  const ring = options.filter((option) => option.text.trim());
  const left = (slot: Slot) => ring.filter((option) => option.slot === slot).length;
  const struck: string[] = [];
  if (loops >= 1) {
    let pos = 0;
    let count = 0;
    while (slots.some((slot) => left(slot) > 1)) {
      if (pos >= ring.length) pos = 0;
      const option = ring[pos]!;
      if (left(option.slot) <= 1) { pos += 1; continue; }
      count += 1;
      if (count === loops) {
        struck.push(option.$id);
        ring.splice(pos, 1);
        count = 0;
      } else {
        pos += 1;
      }
    }
  }
  const picks: Partial<Record<Slot, string>> = {};
  for (const slot of slots) {
    const remaining = ring.filter((option) => option.slot === slot);
    if (remaining.length === 1) picks[slot] = remaining[0]!.$id;
  }
  return { struck, picks };
}

// A spiral of `loops` turns, centred in a 120 square.
export function spiralPath(loops: number): string {
  const turns = Math.max(1, loops);
  const steps = turns * 28;
  let d = "";
  for (let i = 0; i <= steps; i += 1) {
    const angle = (i / steps) * turns * Math.PI * 2;
    const radius = 4 + (i / steps) * 52;
    d += `${i === 0 ? "M" : "L"}${(60 + radius * Math.cos(angle)).toFixed(1)} ${(60 + radius * Math.sin(angle)).toFixed(1)}`;
  }
  return d;
}
