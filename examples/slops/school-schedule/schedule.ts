import { DAYS, isBreakName, type DayKey, type PeriodSlot, type Schedule } from "./schema";

export const DAY_NAMES: Record<DayKey, string> = {
  MON: "Monday",
  TUE: "Tuesday",
  WED: "Wednesday",
  THU: "Thursday",
  FRI: "Friday",
};

export const COLOR_NAMES = {
  sage: "Mint",
  slate: "Sky",
  amber: "Peach",
  terracotta: "Coral",
  indigo: "Lilac",
  rose: "Pink",
  teal: "Aqua",
};

export function minutes(value: string): number | null {
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(value)) return null;
  const [h, m] = value.split(":").map(Number);
  return h * 60 + m;
}

export function validTimes(start: string, end: string): boolean {
  const a = minutes(start);
  const b = minutes(end);
  return a !== null && b !== null && b > a;
}

export function clockLabel(value: string): string {
  const n = minutes(value);
  if (n === null) return "—";
  return `${Math.floor(n / 60) % 12 || 12}:${String(n % 60).padStart(2, "0")}${n < 720 ? "am" : "pm"}`;
}

export function sortedPeriods(periods: readonly PeriodSlot[]): PeriodSlot[] {
  return [...periods].sort((a, b) => (minutes(a.start) ?? 1440) - (minutes(b.start) ?? 1440));
}

export function periodError(period: { periodKey: string; name: string; start: string; end: string }, periods: readonly PeriodSlot[]): string {
  if (!period.name.trim()) return "Give this period a name.";
  if (!validTimes(period.start, period.end)) return "Choose an end time after the start time.";
  const a = minutes(period.start)!;
  const b = minutes(period.end)!;
  if (periods.some((p) => p.periodKey !== period.periodKey && validTimes(p.start, p.end) && a < minutes(p.end)! && b > minutes(p.start)!))
    return "This overlaps another period. Adjust the bell times.";
  return "";
}

export function liveStatus(data: Schedule, now: Date) {
  const day = now.getDay() > 0 && now.getDay() < 6 ? DAYS[now.getDay() - 1] : null;
  if (!day)
    return { day: null, periodKey: null, label: "Weekend", title: "A little time off.", detail: "Your week picks up on Monday.", next: "" };
  const at = now.getHours() * 60 + now.getMinutes();
  const periods = sortedPeriods(data.periods).filter((p) => validTimes(p.start, p.end));
  const events = [
    ...periods.map((p) => {
      const c = data.classes.find((item) => item.day === day && item.periodKey === p.periodKey);
      return {
        start: minutes(p.start)!,
        end: minutes(p.end)!,
        time: p.start,
        title: c?.subject.trim() || (isBreakName(p.name) ? p.name : "Free period"),
        room: c?.room || "",
        periodKey: p.periodKey,
        kind: c?.subject.trim() ? "In class" : isBreakName(p.name) ? p.name : "Free period",
      };
    }),
    ...data.activities
      .filter((a) => a.day === day && validTimes(a.start, a.end))
      .map((a) => ({
        start: minutes(a.start)!,
        end: minutes(a.end)!,
        time: a.start,
        title: a.title || "After-class plans",
        room: a.location,
        periodKey: null,
        kind: "After class",
      })),
  ].sort((a, b) => a.start - b.start);
  const current = events.find((e) => e.start <= at && at < e.end);
  const next = events.find((e) => e.start > at);
  if (current)
    return {
      day,
      periodKey: current.periodKey,
      label: current.kind,
      title: current.title,
      detail: [current.room, `${current.end - at} min left`].filter(Boolean).join(" · "),
      next: next ? `Next: ${next.title}${next.room ? ` · ${next.room}` : ""} · ${clockLabel(next.time)}` : "That’s your last scheduled stop.",
    };
  if (next)
    return {
      day,
      periodKey: null,
      label: events.some((e) => e.end <= at) ? "Up next" : "Before school",
      title: next.title,
      detail: [next.room, `in ${next.start - at} min`].filter(Boolean).join(" · "),
      next: `Starts at ${clockLabel(next.time)}`,
    };
  return {
    day,
    periodKey: null,
    label: events.length ? "All done" : "Open day",
    title: events.length ? "That’s a wrap." : "Room for something good.",
    detail: events.length ? "Nothing else on the schedule today." : "Add a class or an after-class plan.",
    next: "",
  };
}

