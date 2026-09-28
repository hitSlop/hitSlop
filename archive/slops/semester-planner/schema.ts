import { defineDocument, s, type Value } from "@hitslop/document";

export const milestoneTypes = ["Exam", "Paper", "Project", "Presentation", "Break", "Deadline"] as const;

const schema = defineDocument({
  termTitle: s.text(),
  academicYear: s.text(),
  startMonth: s.string(),
  courses: s.list(s.object({
    code: s.string(),
    name: s.text(),
    colorHex: s.string(),
  })),
  milestones: s.list(s.object({
    date: s.string(),
    title: s.text(),
    courseCode: s.string(),
    type: s.enum(milestoneTypes),
    notes: s.text(),
  })),
});

export type Semester = Value<typeof schema.fields.node>;
export type CourseTag = Semester["courses"][number];
export type Milestone = Semester["milestones"][number];
export type MilestoneType = Milestone["type"];
export default schema;
