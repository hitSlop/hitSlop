export function isoDate(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
export function parseTarget(date: string, time: string): Date | null {
  if (
    !/^\d{4}-\d{2}-\d{2}$/.test(date) ||
    !/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(time)
  )
    return null;
  const [y, m, d] = date.split("-").map(Number);
  const [h, min] = time.split(":").map(Number);
  const target = new Date(0);
  target.setFullYear(y, m - 1, d);
  target.setHours(h, min, 0, 0);
  if (
    target.getFullYear() !== y ||
    target.getMonth() !== m - 1 ||
    target.getDate() !== d ||
    target.getHours() !== h ||
    target.getMinutes() !== min
  )
    return null;
  return target;
}
function dayNumber(date: Date) {
  const value = new Date(0);
  value.setUTCFullYear(date.getFullYear(), date.getMonth(), date.getDate());
  value.setUTCHours(0, 0, 0, 0);
  return value.getTime() / 86400000;
}
export type CountdownState = {
  kind: "invalid" | "future" | "today" | "since";
  value: string;
  unit: string;
  detail: string;
  accessible: string;
};
export function countdownState(
  date: string,
  time: string,
  now: number,
): CountdownState {
  const target = parseTarget(date, time);
  if (!target || !Number.isFinite(now))
    return {
      kind: "invalid",
      value: "—",
      unit: "Choose a date",
      detail: "Something to look forward to.",
      accessible: "Choose a valid event date and time",
    };
  const diff = target.getTime() - now;
  if (diff <= 0) {
    const days = Math.max(
      0,
      Math.round(dayNumber(new Date(now)) - dayNumber(target)),
    );
    return days === 0
      ? {
          kind: "today",
          value: "Today",
          unit: "It’s today!",
          detail: "The day you’ve been waiting for.",
          accessible: "The target time has arrived. It’s today.",
        }
      : {
          kind: "since",
          value: String(days),
          unit: days === 1 ? "day since" : "days since",
          detail: "A date worth remembering.",
          accessible: `${days} ${days === 1 ? "day" : "days"} since the event`,
        };
  }
  const seconds = Math.floor(diff / 1000);
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  if (days > 0)
    return {
      kind: "future",
      value: String(days),
      unit: days === 1 ? "day to go" : "days to go",
      detail: `${hours} hr · ${mins} min`,
      accessible: `${days} days, ${hours} hours and ${mins} minutes remaining`,
    };
  if (hours > 0)
    return {
      kind: "future",
      value: String(hours),
      unit: hours === 1 ? "hour to go" : "hours to go",
      detail: `${mins} min`,
      accessible: `${hours} hours and ${mins} minutes remaining`,
    };
  if (seconds >= 60) {
    const minutes = Math.floor(seconds / 60);
    return {
      kind: "future",
      value: String(minutes),
      unit: minutes === 1 ? "minute to go" : "minutes to go",
      detail: "Almost time.",
      accessible: `${minutes} minutes remaining`,
    };
  }
  return {
    kind: "future",
    value: "<1",
    unit: "minute to go",
    detail: "Almost time.",
    accessible: "Less than a minute remaining",
  };
}
export function displayDate(date: string, time: string) {
  const target = parseTarget(date, time);
  return target
    ? new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
        hour: "numeric",
        minute: "2-digit",
      }).format(target)
    : "Choose date & time";
}
