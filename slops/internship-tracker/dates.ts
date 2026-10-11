// Dates are local YYYY-MM-DD strings. Nothing here reads the clock: callers pass "today".
export function dateString(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function dayNumber(date: string): number {
  const [y, m, d] = date.split("-").map(Number);
  return Math.round(Date.UTC(y!, (m ?? 1) - 1, d ?? 1) / 86_400_000);
}

export function daysFromToday(date: string, today: string): number {
  return dayNumber(date) - dayNumber(today);
}

export function dueLabel(date: string | undefined, today: string): { text: string; late: boolean } | null {
  if (!date) return null;
  const diff = daysFromToday(date, today);
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
