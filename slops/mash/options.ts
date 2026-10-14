import { refuse, type RowRef, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { slots } from "./schema";
import { maxOptions, play, slotLabel, type Slot } from "./fate";

export type Mash = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/** The page, the future card and the export derive their blocks and fate from the same snapshot. */
export function mashView(data: Mash) {
  const fate = play(data.options, data.loops);
  const picked = slots.flatMap((slot) => {
    const option = data.options.find((o) => o.$id === fate.picks[slot]);
    return option ? [{ slot, text: option.text }] : [];
  });
  return {
    title: data.title.trim(),
    loops: data.loops,
    fate,
    picked,
    blocks: slots.map((slot) => {
      const options = data.options.filter((option) => option.slot === slot);
      return {
        slot,
        label: slotLabel[slot],
        options,
        filled: options.filter((option) => option.text.trim()),
      };
    }),
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setTitle(tx: Edit, title: string) {
  tx.fields.title.set(title);
}
export function addOption(tx: Edit, slot: Slot, current: Mash) {
  const count = current.options.filter((option) => option.slot === slot).length;
  if (count >= maxOptions) refuse(`${slotLabel[slot]} already has ${maxOptions} options.`);
  return tx.fields.options.insert({ slot, text: "" });
}
export function setOption(tx: Edit, option: RowRef, text: string) {
  tx.fields.options.item(option).text.set(text);
}
export function removeOption(tx: Edit, option: RowRef) {
  tx.fields.options.remove(option);
}
/** Draws 2 to 9 loops from the given source and stores the count; the fate is derived from it. */
export function spin(tx: Edit, random: () => number) {
  const loops = 2 + Math.floor(random() * 8);
  tx.fields.loops.set(loops);
  return loops;
}
