import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  studentName: s.text(),
  term: s.text(),
  courses: s.list(s.object({
    code: s.string(),
    name: s.text(),
    colorHex: s.string(),
  })),
  assignments: s.list(s.object({
    courseCode: s.string(),
    title: s.text(),
    dueDate: s.string(),
    category: s.text(),
    points: s.number({ min: 0 }),
    completed: s.boolean(),
    notes: s.text(),
  })),
});

export type AssignmentTracker = Value<typeof schema.fields.node>;
export type Course = AssignmentTracker["courses"][number];
export type Assignment = AssignmentTracker["assignments"][number];
export default schema;
