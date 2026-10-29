import { refuse, type RowRef, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { kinds } from "./schema";

export type Stash = Value<typeof schema.descriptor>;
export type Product = Stash["products"][number];
export type Kind = Product["kind"];
type Edit = Tx<typeof schema>;

// Product colours are content, not theme: each pan keeps the shade it holds.
export const swatches = ["#e8a0a8", "#d96f7e", "#b8445f", "#e9a07b", "#d9b18f", "#a8704f", "#d8b45a", "#7b3f5e", "#8fa58a", "#7d9bc4"] as const;

export const kindLabel: Record<Kind, string> = {
  face: "Face", cheek: "Cheek", eyes: "Eyes", lips: "Lips", skin: "Skincare", hair: "Hair", body: "Body",
};

export function isKind(value: string): value is Kind {
  return kinds.some((kind) => kind === value);
}

// The metal shows from the centre out; its area equals what has been used.
export function hole(left: number): string {
  return `${Math.round(Math.sqrt(1 - Math.min(100, Math.max(0, left)) / 100) * 100)}%`;
}

function monthIndex(date: string): number {
  const [y, m] = date.split("-").map(Number);
  return y! * 12 + (m ?? 1) - 1;
}

/** The month the clock is in, as a count of months, so expiry never reads the clock itself. */
export function monthsOf(now: Date): number {
  return now.getFullYear() * 12 + now.getMonth();
}

// Months until the period-after-opening ends; negative once it has passed.
export function monthsLeft(product: Product, nowMonths: number): number | undefined {
  if (!product.opened) return undefined;
  return monthIndex(product.opened) + product.pao - nowMonths;
}

export function expiryText(product: Product, nowMonths: number): string {
  const months = monthsLeft(product, nowMonths);
  if (months === undefined) return "Not opened yet";
  if (months < 0) return `Past its ${product.pao}M mark`;
  if (months === 0) return "Use up this month";
  return `${months} month${months === 1 ? "" : "s"} of use left`;
}

export function isExpired(product: Product, nowMonths: number): boolean {
  const months = monthsLeft(product, nowMonths);
  return months !== undefined && months < 0 && product.left > 0;
}

/** The app and its export derive the tray, the shelf and the next pan from the same snapshot. */
export function stashView(data: Stash) {
  const active = data.products.filter((product) => product.left > 0);
  const empties = data.products.filter((product) => product.left === 0);
  // The product closest to empty is the one to reach for next.
  const next = [...active].sort((a, b) => a.left - b.left)[0];
  return { active, empties, next };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addProduct(tx: Edit, input: { name?: string; brand?: string; kind?: Kind; shade?: string; pao?: number }, count: number) {
  if (input.shade !== undefined) checkShade(input.shade);
  return tx.fields.products.insert({
    name: (input.name ?? "").trim(),
    brand: (input.brand ?? "").trim(),
    kind: input.kind ?? "face",
    shade: input.shade ?? swatches[count % swatches.length]!,
    left: 100,
    pao: input.pao ?? 12,
  });
}
export function setLeft(tx: Edit, product: RowRef, left: number) {
  if (!Number.isInteger(left) || left < 0 || left > 100) refuse("Percent left must be a whole number from 0 to 100.");
  tx.fields.products.item(product).left.set(left);
}
export function setOpened(tx: Edit, product: RowRef, date: string) {
  const handle = tx.fields.products.item(product).opened;
  if (!date) return handle.clear();
  if (!/^\d{4}-(0[1-9]|1[0-2])(-\d{2})?$/.test(date)) refuse("Opened must be YYYY-MM-DD or YYYY-MM.");
  handle.set(date);
}
export function setPao(tx: Edit, product: RowRef, months: number) {
  if (!Number.isInteger(months) || months < 1 || months > 60) refuse("Months must be a whole number from 1 to 60.");
  tx.fields.products.item(product).pao.set(months);
}
export function setShade(tx: Edit, product: RowRef, shade: string) {
  checkShade(shade);
  tx.fields.products.item(product).shade.set(shade);
}
export function removeProduct(tx: Edit, product: RowRef) {
  tx.fields.products.remove(product);
}
function checkShade(shade: string) {
  if (!/^#[0-9a-fA-F]{6}$/.test(shade)) refuse("Shade must be a #rrggbb colour.");
}
