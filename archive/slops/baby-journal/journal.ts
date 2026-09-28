import type { Entry } from "./schema";

export function dayKey(value: number | Date): string {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "";
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function localInput(value: number): string {
  const date = new Date(value);
  return `${dayKey(date)}T${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}
export function timeLabel(value: number): string {
  return Number.isFinite(value)
    ? new Date(value).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
    : "Time unavailable";
}
export function dateLabel(day: string): string {
  const date = new Date(`${day}T12:00:00`);
  return Number.isFinite(date.getTime())
    ? date.toLocaleDateString([], { month: "long", day: "numeric", year: "numeric" })
    : "Choose a day";
}
export function duration(minutes: number): string {
  const total = Math.max(0, Math.round(minutes));
  const hours = Math.floor(total / 60);
  return hours ? `${hours}h${total % 60 ? ` ${total % 60}m` : ""}` : `${total} min`;
}
export function elapsed(timestamp: number, now: number): string {
  if (!Number.isFinite(timestamp) || timestamp > now) return "Time unavailable";
  const minutes = Math.floor((now - timestamp) / 60000);
  if (minutes < 1) return "Just now";
  if (minutes >= 1440) return `${Math.floor(minutes / 1440)}d ago`;
  return `${duration(minutes)} ago`;
}
export function entryTitle(entry: Entry): string {
  if (entry.kind === "feeding") {
    if (entry.feedingType === "nursing")
      return `Nursing · ${entry.side === "both" ? "both sides" : `${entry.side} side`}${entry.minutes ? ` · ${duration(entry.minutes)}` : ""}`;
    return `Bottle${entry.amount ? ` · ${entry.amount} ${entry.unit}` : ""}`;
  }
  if (entry.kind === "diaper")
    return `Diaper · ${entry.diaper === "both" ? "wet + dirty" : entry.diaper}`;
  if (entry.kind === "sleep")
    return entry.endTimestamp !== undefined && entry.endTimestamp > entry.timestamp
      ? `Sleep · ${duration((entry.endTimestamp - entry.timestamp) / 60000)}`
      : "Sleep · duration unavailable";
  return "A little memory";
}
export function babyAge(birthday: string, now: number): string {
  if (!birthday) return "One little day at a time";
  const born = new Date(`${birthday}T00:00:00`);
  const today = new Date(now);
  if (!Number.isFinite(born.getTime()) || born > today) return "Birthday unavailable";
  const days = Math.floor(
    (Date.UTC(today.getFullYear(), today.getMonth(), today.getDate()) -
      Date.UTC(born.getFullYear(), born.getMonth(), born.getDate())) /
      86400000,
  );
  if (days < 7) return `${days} ${days === 1 ? "day" : "days"} old`;
  if (days < 84) return `${Math.floor(days / 7)} weeks old`;
  const months =
    (today.getFullYear() - born.getFullYear()) * 12 +
    today.getMonth() -
    born.getMonth() -
    (today.getDate() < born.getDate() ? 1 : 0);
  return months < 24 ? `${months} months old` : `${Math.floor(months / 12)} years old`;
}
