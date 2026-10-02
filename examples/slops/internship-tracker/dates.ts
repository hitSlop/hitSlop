// Dates are stored as local YYYY-MM-DD strings.
export function today(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

function dayNumber(date: string): number {
  const [y, m, d] = date.split("-").map(Number);
  return Math.round(Date.UTC(y!, (m ?? 1) - 1, d ?? 1) / 86_400_000);
}

export function daysFromToday(date: string): number {
  return dayNumber(date) - dayNumber(today());
}

export function dueLabel(date: string | undefined): { text: string; late: boolean } | null {
  if (!date) return null;
  const diff = daysFromToday(date);
  if (diff < 0) return { text: `${-diff}d overdue`, late: true };
  if (diff === 0) return { text: "due today", late: true };
  if (diff === 1) return { text: "due tomorrow", late: false };
  return { text: `due in ${diff}d`, late: false };
}

export function shortDate(date: string | undefined): string {
  if (!date) return "";
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y!, (m ?? 1) - 1, d ?? 1).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
