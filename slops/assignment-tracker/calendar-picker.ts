import { CalendarDate } from "@internationalized/date";
import { formatISO } from "./due";
export function parseISO(iso: string): CalendarDate | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return null;
  const [y, m, d] = iso.split("-").map(Number);
  if (y < 1 || m < 1 || m > 12 || d < 1 || d > 31) return null;
  try {
    const date = new CalendarDate(y, m, d);
    return formatISO(date) === iso ? date : null;
  } catch {
    return null;
  }
}
