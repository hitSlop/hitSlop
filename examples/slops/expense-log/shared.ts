export const CATEGORIES = [
  { id: "food", label: "Food", code: "FOOD" },
  { id: "transit", label: "Transit", code: "TRANSIT" },
  { id: "coffee", label: "Coffee", code: "COFFEE" },
  { id: "gear", label: "Gear", code: "GEAR" },
  { id: "bills", label: "Bills", code: "BILLS" },
  { id: "other", label: "Other", code: "MISC" },
] as const;

export function formatDate(d: Date): string {
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function categoryCode(id: string): string {
  return CATEGORIES.find((cat) => cat.id === id)?.code ?? "MISC";
}

export const today = formatDate(new Date());

export function formatAmount(amount: number, symbol: string): string {
  return `${symbol}${amount.toFixed(2)}`;
}

export function receiptLabel(title: string): string {
  return title.trim().toUpperCase().slice(0, 12) || "ITEM";
}
