import { defineDocument, s } from "hitslop";

export const tapes = ["pink", "mint", "butter", "lilac", "sky", "peach"] as const;
export const stickers = ["star", "heart", "bolt", "fire", "crown", "ghost", "cherry", "sparkle", "frog", "rainbow"] as const;
export const sprites = ["cat", "frog", "bunny", "ghost", "alien", "bear"] as const;

const quest = (title: string, due: string, done = false, notes = "") =>
  ({ title, due, notes, ...(done ? { done, sticker: "star" as const } : {}) });
const boss = (title: string, due: string, maxHp: number, hits: number, notes = "") =>
  ({ title, kind: "boss" as const, due, maxHp, hits, notes });

export default defineDocument({
  semester: s.text(),
  player: s.object({ name: s.text(), sprite: s.enum(sprites) }),
  courses: s.list(s.object({
    code: s.text(),
    name: s.text(),
    tape: s.enum(tapes),
    quests: s.list(s.object({
      title: s.text(),
      kind: s.enum(["quest", "boss"], { default: "quest" }),
      due: s.string(),
      done: s.boolean({ default: false }),
      maxHp: s.integer({ min: 1, max: 50, default: 1 }),
      hits: s.counter(),
      notes: s.text(),
      sticker: s.optional(s.enum(stickers)),
    })),
  })),
  sideQuests: s.list(s.object({ title: s.text(), done: s.boolean({ default: false }) })),
  placed: s.list(s.object({ sticker: s.enum(stickers), x: s.number(), y: s.number(), turn: s.number() })),
}, {
  initial: {
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
      { title: "Call mom" },
      { title: "Gym 3×/week" },
      { title: "Buy a real winter coat" },
    ],
    placed: [{ sticker: "star", x: 0.12, y: 0.08, turn: -8 }],
  },
});
