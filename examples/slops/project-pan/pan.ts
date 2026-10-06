import type { Product } from "./schema";

// Product colours are content, not theme: each pan keeps the shade it holds.
export const swatches = ["#e8a0a8", "#d96f7e", "#b8445f", "#e9a07b", "#d9b18f", "#a8704f", "#d8b45a", "#7b3f5e", "#8fa58a", "#7d9bc4"] as const;

export const kindLabel: Record<Product["kind"], string> = {
  face: "Face", cheek: "Cheek", eyes: "Eyes", lips: "Lips", skin: "Skincare", hair: "Hair", body: "Body",
};

// The metal shows from the centre out; its area equals what has been used.
export function hole(left: number): string {
  return `${Math.round(Math.sqrt(1 - Math.min(100, Math.max(0, left)) / 100) * 100)}%`;
}

function monthIndex(date: string): number {
  const [y, m] = date.split("-").map(Number);
  return y! * 12 + (m ?? 1) - 1;
}

function todayMonths(): number {
  const now = new Date();
  return now.getFullYear() * 12 + now.getMonth();
}

// Months until the period-after-opening ends; negative once it has passed.
export function monthsLeft(product: Product): number | undefined {
  if (!product.opened) return undefined;
  return monthIndex(product.opened) + product.pao - todayMonths();
}

export function expiryText(product: Product): string {
  const months = monthsLeft(product);
  if (months === undefined) return "Not opened yet";
  if (months < 0) return `Past its ${product.pao}M mark`;
  if (months === 0) return "Use up this month";
  return `${months} month${months === 1 ? "" : "s"} of use left`;
}

export function isExpired(product: Product): boolean {
  const months = monthsLeft(product);
  return months !== undefined && months < 0 && product.left > 0;
}

export function panned(products: readonly Product[]): Product[] {
  return products.filter((product) => product.left === 0);
}

export function inUse(products: readonly Product[]): Product[] {
  return products.filter((product) => product.left > 0);
}

// The product closest to empty is the one to reach for next.
export function nextUp(products: readonly Product[]): Product | undefined {
  return [...inUse(products)].sort((a, b) => a.left - b.left)[0];
}
