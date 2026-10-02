import doc from "./schema";

export function caregiverLabel(id: string) {
  return doc.current.caregivers.find(person => person.$id === id)?.name.trim() || "Unknown caregiver";
}

export function caregiverInitials(id: string) {
  return caregiverLabel(id).split(/\s+/).slice(0, 2).map(part => part[0]).join("").toLocaleUpperCase();
}
