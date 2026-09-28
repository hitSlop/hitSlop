import { defineDocument, s, type Value } from "@hitslop/document";

export const tapes = ["pink", "mint", "butter", "lilac", "sky", "peach"] as const;
export const stickers = ["star", "heart", "bolt", "fire", "crown", "ghost", "cherry", "sparkle", "frog", "rainbow"] as const;
export const sprites = ["cat", "frog", "bunny", "ghost", "alien", "bear"] as const;
export type Tape = (typeof tapes)[number];
export type Sticker = (typeof stickers)[number];
export type Sprite = (typeof sprites)[number];

const schema = defineDocument({
  semester: s.text(),
  player: s.object({ name: s.text(), sprite: s.enum(sprites) }),
  courses: s.list(s.object({
    code: s.text(),
    name: s.text(),
    tape: s.enum(tapes),
    quests: s.list(s.object({
      title: s.text(),
      kind: s.enum(["quest", "boss"]),
      due: s.string(),
      done: s.boolean(),
      maxHp: s.integer({ min: 1, max: 50 }),
      hits: s.counter(),
      notes: s.text(),
      sticker: s.optional(s.enum(stickers)),
    })),
  })),
  sideQuests: s.list(s.object({ title: s.text(), done: s.boolean() })),
  placed: s.list(s.object({ sticker: s.enum(stickers), x: s.number(), y: s.number(), turn: s.number() })),
});

export type Map = Value<typeof schema.fields.node>;
export type Course = Map["courses"][number];
export type Quest = Course["quests"][number];
export type SideQuest = Map["sideQuests"][number];
export type Placed = Map["placed"][number];
export default schema;
