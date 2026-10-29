import { defineDocument, s } from "@hitslop/tinys";

export default defineDocument({
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
    dueDate: s.string({ description: "Local YYYY-MM-DD, or empty when undated." }),
    category: s.text(),
    points: s.number({ min: 0 }),
    completed: s.boolean({ default: false }),
    notes: s.text(),
  })),
}, { initial:  {
  "studentName": "Alex Rivera",
  "term": "Fall Semester 2026",
  "courses": [
    {
      "code": "BIO",
      "name": "AP Biology",
      "colorHex": "#3b7a57"
    },
    {
      "code": "CALC",
      "name": "Pre-Calculus",
      "colorHex": "#4f46e5"
    },
    {
      "code": "LIT",
      "name": "Honors Lit",
      "colorHex": "#4a5d6e"
    },
    {
      "code": "HIST",
      "name": "World History",
      "colorHex": "#d97706"
    },
    {
      "code": "SPAN",
      "name": "Spanish III",
      "colorHex": "#c2410c"
    }
  ],
  "assignments": [
    {
      "courseCode": "BIO",
      "title": "Cellular Respiration Lab Writeup",
      "dueDate": "2026-09-16",
      "category": "Lab Report",
      "points": 50,
      "notes": "Include graph for trial 3 enzyme rate"
    },
    {
      "courseCode": "CALC",
      "title": "Problem Set 2.4: Trigonometric Identities #1–24",
      "dueDate": "2026-09-16",
      "category": "Problem Set",
      "points": 20,
      "notes": "Show all unit circle substitutions"
    },
    {
      "courseCode": "LIT",
      "title": "Read The Great Gatsby Chapters 1–3",
      "dueDate": "2026-09-18",
      "category": "Reading",
      "points": 15,
      "notes": "Annotate the green light motif"
    },
    {
      "courseCode": "HIST",
      "title": "DBQ Outline: Industrial Revolution",
      "dueDate": "2026-09-20",
      "category": "Essay",
      "points": 40,
      "notes": "Draft a thesis before Friday"
    },
    {
      "courseCode": "SPAN",
      "title": "Subjunctive workbook p.42–45",
      "dueDate": "2026-09-17",
      "category": "Homework",
      "points": 10,
      "completed": true,
      "notes": ""
    },
    {
      "courseCode": "BIO",
      "title": "Unit 1 Quiz Prep: Macromolecules",
      "dueDate": "2026-09-21",
      "category": "Quiz Prep",
      "points": 30,
      "notes": "Review Leitner box 2"
    }
  ]
} });
