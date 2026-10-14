import type { Caregiver } from "./journal";

/** Pure lookups; callers pass the caregivers from the document snapshot. */
export function caregiverLabel(people: readonly Caregiver[], id: string) {
  return people.find(person => person.$id === id)?.name.trim() || "Unknown caregiver";
}

export function caregiverInitials(people: readonly Caregiver[], id: string) {
  return caregiverLabel(people, id).split(/\s+/).slice(0, 2).map(part => part[0]).join("").toLocaleUpperCase();
}
