import { defineDocument, s, type Value } from "@hitslop/document";

const category = s.object({
  name: s.text(),
  weightPercent: s.number(),
  scorePercent: s.optional(s.number()),
  isFinal: s.boolean(),
});

const schema = defineDocument({
  studentName: s.text(),
  term: s.text(),
  courses: s.list(s.object({
    code: s.text(),
    name: s.text(),
    credits: s.number(),
    targetPercent: s.number(),
    categories: s.list(category),
  })),
});

export type GradeBook = Value<typeof schema.fields.node>;
export type CourseGrade = GradeBook["courses"][number];
export type CategoryWeight = CourseGrade["categories"][number];
export default schema;
