import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  name: s.text(),
  initials: s.string({ maxLength: 3 }),
  role: s.text(),
  email: s.string(),
  location: s.text(),
  website: s.string(),
  summary: s.text(),
  skills: s.list(s.object({ label: s.text() })),
  experience: s.list(s.object({
    role: s.text(),
    company: s.text(),
    period: s.text(),
    summary: s.text(),
  })),
  education: s.list(s.object({
    school: s.text(),
    program: s.text(),
    year: s.string(),
  })),
});

export type Resume = Value<typeof schema.fields.node>;
export default schema;

export function badgeInitials(name: string, initials: string): string {
  const fromName = name.split(/\s+/).filter(Boolean).map(word => word[0] ?? "").join("").slice(0, 2).toUpperCase();
  return (initials.trim() || fromName || "AQ").slice(0, 2).toUpperCase();
}
