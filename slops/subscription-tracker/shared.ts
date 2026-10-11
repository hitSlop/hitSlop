// Pure helpers: every date is passed in, so nothing here reads the clock.

export function localDate(date: Date): string {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
}

export function isDate(value: string): boolean {
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(new Date(`${value}T12:00:00`).getTime());
}

export function dateLabel(value: string): string {
  if (!isDate(value)) return "No date";
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" }).format(new Date(`${value}T12:00:00`));
}

export function renewalState(value: string, today: string): "overdue" | "soon" | "normal" {
  if (!isDate(value)) return "normal";
  if (value < today) return "overdue";
  const soon = new Date(`${today}T12:00:00`);
  soon.setDate(soon.getDate() + 30);
  return value <= localDate(soon) ? "soon" : "normal";
}

export function money(value: number, currency: string): string {
  return new Intl.NumberFormat(undefined, { style: "currency", currency }).format(value);
}

export function cadenceLabel(cadence: string): string {
  return cadence === "annual" ? "yr" : "mo";
}
