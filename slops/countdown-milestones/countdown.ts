import { dateLabel } from "../_shared/format";
import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
export type Countdown = typeof doc.current;
type Edit = Tx<typeof doc>;
export const maxMilestones = 25;

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
    ? dateLabel(target, {
        month: "short",
        day: "numeric",
        year: "numeric",
        hour: "numeric",
        minute: "2-digit",
      })
    : "Choose date & time";
}

/** The editor and its captures derive their numbers from the same snapshot; `now` is passed in. */
export function countdownView(data: Countdown, now: number) {
  const done = data.milestones.filter(item => item.done).length;
  const total = data.milestones.length;
  return {
    state: countdownState(data.targetDate, data.targetTime, now),
    done, total,
    ratio: total ? done / total * 100 : 0,
    full: total >= maxMilestones,
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setCountdown(tx: Edit, title: string, date: string, time: string) {
  const name = title.trim();
  if (!name) refuse("Give your event a name.");
  if (!parseTarget(date, time)) refuse("Choose a valid date (YYYY-MM-DD) and local time (HH:MM).");
  tx.fields.title.set(name);
  tx.fields.targetDate.set(date);
  tx.fields.targetTime.set(time);
}
export function addMilestone(tx: Edit, current: Countdown, title: string) {
  const name = title.trim();
  if (!name) refuse("Enter a milestone.");
  if (current.milestones.length >= maxMilestones) refuse(`A countdown holds at most ${maxMilestones} milestones.`);
  return tx.fields.milestones.insert({ title: name });
}
export function setMilestoneDone(tx: Edit, id: string, done: boolean) {
  tx.fields.milestones.item(id).done.set(done);
}
export function moveMilestone(tx: Edit, current: Countdown, id: string, step: number) {
  const items = current.milestones;
  const index = items.findIndex(item => item.$id === id);
  if (index < 0) refuse("No such milestone.");
  const neighbor = items[index + step];
  if (!neighbor) refuse("The milestone cannot move further that way.");
  tx.fields.milestones.move(id, step < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
}
export function removeMilestone(tx: Edit, id: string) {
  tx.fields.milestones.remove(id);
}
