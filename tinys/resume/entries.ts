import type { Tx, Value } from "@hitslop/tinys";
import type schema from "./schema";
type Edit = Tx<typeof schema>;
type Data = Value<typeof schema.descriptor>;

export function badgeInitials(name: string, initials: string): string {
  const fromName = name.split(/\s+/).filter(Boolean).map(word => word[0] ?? "").join("").slice(0, 2).toUpperCase();
  return (initials.trim() || fromName || "AQ").slice(0, 2).toUpperCase();
}

/** The editor and the export derive their lists and monogram from the same snapshot. */
export function resumeView(data: Data) {
  const skills = data.skills;
  const education = data.education;
  const experience = data.experience;
  return {
    initials: badgeInitials(data.name, data.initials),
    skills,
    education,
    experience,
    // Export shows only entries that have something in them.
    printed: {
      skills: skills.filter(skill => skill.label.trim()),
      education: education.filter(item => item.school.trim() || item.program.trim() || item.year.trim()),
      experience: experience.filter(item => item.role.trim() || item.company.trim() || item.period.trim() || item.summary.trim()),
    },
  };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function addSkill(tx: Edit) {
  return tx.fields.skills.insert({ label: "New skill" });
}
export function removeSkill(tx: Edit, id: string) {
  tx.fields.skills.remove(id);
}
export function addExperience(tx: Edit) {
  return tx.fields.experience.insert({ role: "New role", company: "Company", period: "Year — now", summary: "A short description of the work and the impact you made." });
}
export function removeExperience(tx: Edit, id: string) {
  tx.fields.experience.remove(id);
}
export function addEducation(tx: Edit) {
  return tx.fields.education.insert({ school: "School or program", program: "Degree or course", year: "Year" });
}
export function removeEducation(tx: Edit, id: string) {
  tx.fields.education.remove(id);
}
