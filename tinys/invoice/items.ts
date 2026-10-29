import type { Tx, Value } from "@hitslop/tinys";
import type schema from "./schema";
export type Invoice = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

/** The editor and the export page derive their totals from the same snapshot. */
export function invoiceView(data: Invoice) {
  const subtotal = data.items.reduce((sum, item) => sum + item.quantity * item.rate, 0);
  const tax = subtotal * data.taxPercent / 100;
  return { subtotal, tax, total: subtotal + tax };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addItem(tx: Edit) {
  return tx.fields.items.insert({ description: "", quantity: 1, rate: 0 });
}
export function removeItem(tx: Edit, id: string) {
  tx.fields.items.remove(id);
}
export function setStatus(tx: Edit, status: Invoice["status"]) {
  tx.fields.status.set(status);
}
export function setCurrency(tx: Edit, currency: Invoice["currency"]) {
  tx.fields.currency.set(currency);
}
export function setTaxPercent(tx: Edit, percent: number) {
  tx.fields.taxPercent.set(percent);
}
