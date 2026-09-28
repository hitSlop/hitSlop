import type { Input } from "@hitslop/document";
import schema from "./schema";

const quest = (title: string, due: string, done = false, notes = "") =>
  ({ title, kind: "quest" as const, due, done, maxHp: 1, hits: 0, notes, ...(done ? { sticker: "star" as const } : {}) });
const boss = (title: string, due: string, maxHp: number, hits: number, notes = "") =>
  ({ title, kind: "boss" as const, due, done: false, maxHp, hits, notes });

export default {
  semester: "Fall '26",
  player: { name: "me", sprite: "cat" },
  courses: [
    {
      code: "CHEM 101", name: "Intro Chemistry", tape: "mint",
      quests: [
        quest("Lab 1 write-up", "2026-09-12", true),
        quest("Problem set 3", "2026-09-26"),
        boss("Midterm", "2026-10-09", 8, 3, "Chapters 1–5. Flashcards for reaction types."),
        quest("Lab 4 write-up", "2026-10-24"),
        boss("Final", "2026-12-11", 12, 0),
      ],
    },
    {
      code: "PSYC 110", name: "Intro Psychology", tape: "pink",
      quests: [
        quest("Reading response 1", "2026-09-15", true),
        quest("Reading response 2", "2026-09-29"),
        quest("Group project pitch", "2026-10-16"),
        boss("Midterm", "2026-10-28", 6, 0),
        quest("Research paper", "2026-11-20"),
      ],
    },
    {
      code: "CS 106", name: "Programming Methods", tape: "sky",
      quests: [
        quest("Assignment 1: Karel", "2026-09-18", true),
        quest("Assignment 2: Strings", "2026-10-02"),
        quest("Assignment 3: Recursion", "2026-10-20"),
        boss("Midterm", "2026-11-04", 10, 1),
        quest("Final project", "2026-12-04"),
      ],
    },
  ],
  sideQuests: [
    { title: "Club fair", done: true },
    { title: "Call mom", done: false },
    { title: "Gym 3×/week", done: false },
    { title: "Buy a real winter coat", done: false },
  ],
  placed: [{ sticker: "star", x: 0.12, y: 0.08, turn: -8 }],
} satisfies Input<typeof schema.fields.node>;
