import { refuse, type RowRef, type Tx } from "hitslop";
import type doc from "./schema";
import type { stages } from "./schema";
import { daysFromToday } from "./dates";

export type Tracker = typeof doc.current;
export type Stage = (typeof stages)[number];
type Edit = Tx<typeof doc>;

/** The badge and the export derive counts, nudges and rows from the same snapshot. */
export function trackerView(data: Tracker, today: string) {
  const nudges = data.applications.filter((app) => needsNudge(app, today)).length;
  return {
    title: data.title,
    applications: data.applications,
    chats: data.chats,
    counts: stageCounts(data.applications),
    nudges,
    nudgeLabel: nudges > 0 ? `${nudges} follow-up${nudges === 1 ? "" : "s"} due` : "No follow-ups due",
  };
}

export function stageCounts(applications: Tracker["applications"]): Record<Stage, number> {
  const counts: Record<Stage, number> = { wishlist: 0, applied: 0, assessment: 0, interview: 0, offer: 0, rejected: 0 };
  for (const app of applications) counts[app.stage] += 1;
  return counts;
}

// A follow-up is due once its date arrives, unless the application is settled.
export function needsNudge(app: Tracker["applications"][number], today: string): boolean {
  if (app.stage === "offer" || app.stage === "rejected" || !app.followUp) return false;
  return daysFromToday(app.followUp, today) <= 0;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
function checkDate(value: string) {
  if (value && !/^\d{4}-\d{2}-\d{2}$/.test(value)) refuse("Use a YYYY-MM-DD date.");
}
export function addApplication(tx: Edit, company: string, role: string) {
  const name = company.trim();
  if (!name) refuse("Enter a company.");
  return tx.fields.applications.insert({ company: name, role: role.trim(), stage: "applied" });
}
export function setStage(tx: Edit, application: RowRef, stage: Stage) {
  tx.fields.applications.item(application).stage.set(stage);
}
export function setFollowUp(tx: Edit, application: RowRef, date: string) {
  checkDate(date);
  const handle = tx.fields.applications.item(application).followUp;
  if (date) handle.set(date);
  else handle.clear();
}
export function removeApplication(tx: Edit, application: RowRef) {
  tx.fields.applications.remove(application);
}
export function addChat(tx: Edit, name: string, place: string) {
  const who = name.trim();
  if (!who) refuse("Enter a name.");
  return tx.fields.chats.insert({ name: who, place: place.trim() });
}
export function setThanked(tx: Edit, chat: RowRef, thanked: boolean) {
  tx.fields.chats.item(chat).thanked.set(thanked);
}
export function setMetDate(tx: Edit, chat: RowRef, date: string) {
  checkDate(date);
  const handle = tx.fields.chats.item(chat).when;
  if (date) handle.set(date);
  else handle.clear();
}
export function removeChat(tx: Edit, chat: RowRef) {
  tx.fields.chats.remove(chat);
}
