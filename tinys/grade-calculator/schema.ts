import { defineDocument, s } from "@hitslop/tinys";

const category = s.object({
  name: s.text(),
  weightPercent: s.number(),
  scorePercent: s.optional(s.number()),
  isFinal: s.boolean({ default: false }),
});

export default defineDocument({
  studentName: s.text(),
  term: s.text(),
  courses: s.list(s.object({
    code: s.text(),
    name: s.text(),
    credits: s.number(),
    targetPercent: s.number(),
    categories: s.list(category),
  })),
}, { initial: {
  studentName: "Alex Rivera",
  term: "Fall Semester 2026",
  courses: [
    {
      code: "BIO 101",
      name: "AP Biology",
      credits: 4,
      targetPercent: 90,
      categories: [
        { name: "Homework & Problem Sets", weightPercent: 20, scorePercent: 95 },
        { name: "Lab Reports", weightPercent: 25, scorePercent: 92 },
        { name: "Midterm Exams", weightPercent: 25, scorePercent: 88 },
        { name: "Comprehensive Final Exam", weightPercent: 30, isFinal: true },
      ],
    },
    {
      code: "MATH 150",
      name: "Pre-Calculus",
      credits: 4,
      targetPercent: 90,
      categories: [
        { name: "Weekly Quizzes", weightPercent: 25, scorePercent: 86 },
        { name: "Midterm 1 & 2", weightPercent: 40, scorePercent: 89 },
        { name: "Final Exam", weightPercent: 35, isFinal: true },
      ],
    },
    {
      code: "ENG 201",
      name: "Honors Literature",
      credits: 3,
      targetPercent: 93,
      categories: [
        { name: "Essays & Papers", weightPercent: 50, scorePercent: 96 },
        { name: "Seminar Participation", weightPercent: 20, scorePercent: 98 },
        { name: "Final Portfolio", weightPercent: 30, isFinal: true },
      ],
    },
    {
      code: "HIST 110",
      name: "World History",
      credits: 3,
      targetPercent: 85,
      categories: [
        { name: "Primary Source DBQs", weightPercent: 35, scorePercent: 84 },
        { name: "Unit Tests", weightPercent: 35, scorePercent: 82 },
        { name: "Semester Exam", weightPercent: 30, isFinal: true },
      ],
    },
  ],
} });
