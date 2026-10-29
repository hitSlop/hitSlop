import { dateLabel } from "../_shared/format";
import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { kinds, maxDice, maxModifier, maxRolls, type Kind } from "./schema";

export type Tray = Value<typeof schema.descriptor>;
export type Roll = Tray["rolls"][number];
type Edit = Tx<typeof schema>;
export type Pool = Record<Kind, number>;

export const sides: Record<Kind, number> = { d4: 4, d6: 6, d8: 8, d10: 10, d12: 12, d20: 20 };

export function poolOf(data: Tray): Pool {
  return Object.fromEntries(kinds.map((kind) => [kind, data.pool[kind]])) as Pool;
}
export function diceCount(pool: Pool): number {
  return kinds.reduce((sum, kind) => sum + pool[kind], 0);
}

/** "1d20 + 1d4 + 5": largest dice first, as players say it. */
export function formula(pool: Pool, modifier: number): string {
  const terms = [...kinds].reverse().filter((kind) => pool[kind] > 0).map((kind) => `${pool[kind]}${kind}`);
  if (modifier) terms.push(String(Math.abs(modifier)));
  if (!terms.length) return "No dice";
  let text = terms[0]!;
  for (let i = 1; i < terms.length; i++) {
    const negative = i === terms.length - 1 && modifier < 0;
    text += `${negative ? " − " : " + "}${terms[i]}`;
  }
  if (terms.length === 1 && modifier < 0) text = `−${text}`;
  return text;
}

export function rollPool(roll: Roll): Pool {
  const pool = Object.fromEntries(kinds.map((kind) => [kind, 0])) as Pool;
  for (const die of roll.dice) pool[die.kind]++;
  return pool;
}

/** One roll as the readout, the history and the export show it. */
export function rollView(roll: Roll) {
  const sum = roll.dice.reduce((total, die) => total + die.value, 0);
  const single = roll.dice.length === 1 ? roll.dice[0] : undefined;
  return {
    id: roll.$id,
    total: sum + roll.modifier,
    formula: formula(rollPool(roll), roll.modifier),
    values: roll.dice.map((die) => ({ kind: die.kind, value: die.value })),
    // "17 + 3 + 5": each die as it came up, then the modifier.
    breakdown: [
      roll.dice.map((die) => die.value).join(" + "),
      roll.modifier ? `${roll.modifier < 0 ? "−" : "+"} ${Math.abs(roll.modifier)}` : "",
    ].filter(Boolean).join(" "),
    modifier: roll.modifier,
    at: roll.at,
    // A lone d20 that shows 20 or 1 is the moment people call out at the table.
    natural: single?.kind === "d20" && (single.value === 20 || single.value === 1) ? single.value : undefined,
  };
}
export type RollView = ReturnType<typeof rollView>;

/** "8:07 PM" for a roll today, "Oct 8, 8:07 PM" otherwise, relative to `now`. */
export function formatWhen(at: string, now: number): string {
  const when = new Date(at);
  if (Number.isNaN(when.getTime())) return "";
  const time = dateLabel(when, { hour: "numeric", minute: "2-digit" });
  return when.toDateString() === new Date(now).toDateString()
    ? time
    : `${dateLabel(when, { month: "short", day: "numeric" })}, ${time}`;
}

/** The editor and its export derive the readout and history from the same snapshot. */
export function trayView(data: Tray) {
  const pool = poolOf(data);
  const history = data.rolls.map(rollView).reverse();
  return {
    title: data.title.trim() || "Dice Tray",
    pool,
    dice: diceCount(pool),
    full: diceCount(pool) >= maxDice,
    next: formula(pool, data.modifier),
    last: data.rolls.at(-1),
    lastView: history[0],
    history,
  };
}

/** Parse "2d6+3", "d20 - 1" or "1d8 + 3d6 + 4" into a pool and modifier, or say what is wrong. */
export function parseFormula(text: string): { pool: Pool; modifier: number } | string {
  const pool = Object.fromEntries(kinds.map((kind) => [kind, 0])) as Pool;
  let modifier = 0;
  const compact = text.replace(/\s+/g, "").replace(/−/g, "-").toLowerCase();
  if (!compact) return "Write dice like 2d6+3.";
  const pattern = /([+-]?)(\d*)(d(\d+))?/y;
  let at = 0;
  while (at < compact.length) {
    pattern.lastIndex = at;
    const match = pattern.exec(compact);
    if (!match || match[0] === "" || match[0] === "+" || match[0] === "-") return `Couldn't read "${text}". Write dice like 2d6+3.`;
    at = pattern.lastIndex;
    const sign = match[1] === "-" ? -1 : 1;
    if (match[3]) {
      if (sign < 0) return "Dice can only be added, not subtracted.";
      const kind = `d${Number(match[4])}` as Kind;
      if (!kinds.includes(kind)) return `There's no d${match[4]} in the tray: use d4, d6, d8, d10, d12 or d20.`;
      pool[kind] += match[2] === "" ? 1 : Number(match[2]);
    } else {
      modifier += sign * Number(match[2]);
    }
  }
  return poolProblem(pool, modifier) ?? { pool, modifier };
}

function poolProblem(pool: Pool, modifier: number): string | undefined {
  if (diceCount(pool) > maxDice) return `The tray holds ${maxDice} dice at most.`;
  if (!Number.isInteger(modifier) || Math.abs(modifier) > maxModifier) return `Keep the modifier between −${maxModifier} and +${maxModifier}.`;
  return undefined;
}

/** A random source the page can hand to `roll`: the browser's cryptographic generator. */
export function fairRandom(): number {
  const word = new Uint32Array(1);
  crypto.getRandomValues(word);
  return word[0]! / 4294967296;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

export function setPool(tx: Edit, pool: Pool, modifier: number) {
  const problem = poolProblem(pool, modifier);
  if (problem) refuse(problem);
  for (const kind of kinds) tx.fields.pool[kind].set(pool[kind]);
  tx.fields.modifier.set(modifier);
  return formula(pool, modifier);
}

export function setFormula(tx: Edit, text: string) {
  const parsed = parseFormula(text);
  if (typeof parsed === "string") refuse(parsed);
  return setPool(tx, parsed.pool, parsed.modifier);
}

export function addDie(tx: Edit, data: Tray, kind: Kind) {
  if (!kinds.includes(kind)) refuse(`Choose one of: ${kinds.join(", ")}.`);
  if (diceCount(poolOf(data)) >= maxDice) refuse(`The tray holds ${maxDice} dice at most.`);
  tx.fields.pool[kind].set(data.pool[kind] + 1);
}

export function removeDie(tx: Edit, data: Tray, kind: Kind) {
  if (data.pool[kind] > 0) tx.fields.pool[kind].set(data.pool[kind] - 1);
}

export function setModifier(tx: Edit, modifier: number) {
  if (!Number.isInteger(modifier) || Math.abs(modifier) > maxModifier) refuse(`Keep the modifier between −${maxModifier} and +${maxModifier}.`);
  tx.fields.modifier.set(modifier);
}

/**
 * Roll the pool: every value is drawn from `random` (0 ≤ r < 1) and stored with the time,
 * the modifier and a seed for the tumble. Returns the new roll's $id and what came up.
 */
export function roll(tx: Edit, data: Tray, random: () => number, now: number) {
  const pool = poolOf(data);
  if (!diceCount(pool)) refuse("Add a die to the pool first.");
  const dice = [...kinds].reverse().flatMap((kind) =>
    Array.from({ length: pool[kind] }, () => ({ kind, value: 1 + Math.min(sides[kind] - 1, Math.floor(random() * sides[kind])) })),
  );
  const seed = Math.floor(random() * 2147483647);
  const { id } = tx.fields.rolls.insert({ at: new Date(now).toISOString(), seed, modifier: data.modifier, dice });
  // Keep the history short enough to read: drop the oldest beyond the cap.
  const excess = data.rolls.length + 1 - maxRolls;
  for (let i = 0; i < excess; i++) tx.fields.rolls.remove(data.rolls[i]!.$id);
  const total = dice.reduce((sum, die) => sum + die.value, 0) + data.modifier;
  return { id, total, values: dice.map((die) => `${die.kind}:${die.value}`) };
}

/** Make `pool` and `modifier` the pool, then roll them. */
function rollWith(tx: Edit, data: Tray, pool: Pool, modifier: number, random: () => number, now: number) {
  setPool(tx, pool, modifier);
  return roll(tx, { ...data, pool, modifier } as Tray, random, now);
}

/** Make a formula like "2d6+3" the pool, then roll it. */
export function rollFormula(tx: Edit, data: Tray, text: string, random: () => number, now: number) {
  const parsed = parseFormula(text);
  if (typeof parsed === "string") refuse(parsed);
  return rollWith(tx, data, parsed.pool, parsed.modifier, random, now);
}

/** Make an earlier roll's dice and modifier the pool again, then roll them. */
export function rollAgain(tx: Edit, data: Tray, rollId: string, random: () => number, now: number) {
  const earlier = data.rolls.find((row) => row.$id === rollId) ?? refuse("That roll is no longer in the history.");
  return rollWith(tx, data, rollPool(earlier), earlier.modifier, random, now);
}

export function clearHistory(tx: Edit, data: Tray) {
  for (const row of data.rolls) tx.fields.rolls.remove(row.$id);
  return data.rolls.length;
}

export function rename(tx: Edit, title: string) {
  tx.fields.title.set(title.trim().slice(0, 40));
}
