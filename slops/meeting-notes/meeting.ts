import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";

type Edit = Tx<typeof schema>;

/** The editor and its export derive counts and listed attendees from the same snapshot. */
export function meetingView(data: Value<typeof schema.descriptor>) {
  return {
    agendaDone: data.agenda.filter((topic) => topic.done).length,
    actionsDone: data.actions.filter((action) => action.done).length,
    listedAttendees: data.attendees.filter((person) => person.trim()),
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addAttendee(tx: Edit, name: string) {
  const person = name.trim();
  if (!person) refuse("Enter a name.");
  tx.fields.attendees.insert(person);
}
export function removeAttendee(tx: Edit, index: number) {
  tx.fields.attendees.remove(index);
}
export function addAgendaTopic(tx: Edit, text = "") {
  return tx.fields.agenda.insert({ text });
}
export function removeAgendaTopic(tx: Edit, topic: string) {
  tx.fields.agenda.remove(topic);
}
export function addDecision(tx: Edit, text = "") {
  return tx.fields.decisions.insert({ text });
}
export function removeDecision(tx: Edit, decision: string) {
  tx.fields.decisions.remove(decision);
}
export function addAction(tx: Edit, text: string, owner: string) {
  return tx.fields.actions.insert({ text, owner });
}
export function setActionDone(tx: Edit, action: string, done: boolean) {
  tx.fields.actions.item(action).done.set(done);
}
export function removeAction(tx: Edit, action: string) {
  tx.fields.actions.remove(action);
}
