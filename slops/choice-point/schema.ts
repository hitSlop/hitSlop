import { defineDocument, s } from "hitslop";

const item = s.object({ text: s.text() });

export const lists = ["hooks", "awayMoves", "towardsMoves", "helpers"] as const;

export default defineDocument({
  date: s.text(),
  situation: s.text(),
  hooks: s.list(item),
  awayMoves: s.list(item),
  towardsMoves: s.list(item),
  helpers: s.list(item),
  nextMove: s.text(),
  nextWhen: s.text(),
  nextDone: s.boolean({ default: false }),
}, { initial: {
  date: "Fri, Sep 25, 2026",
  situation: "Feedback landed hard in the team review.",
  hooks: [{ text: "“I should already know this”" }, { text: "Hot face, tight chest" }, { text: "Urge to shut down" }],
  awayMoves: [{ text: "Put off the revision" }, { text: "Get short with my teammates" }],
  towardsMoves: [{ text: "Thank them for catching it" }, { text: "Pair on the first fix" }],
  helpers: [{ text: "Craft" }, { text: "Humility" }, { text: "Three slow breaths" }, { text: "Name it: “here’s the not-good-enough story”" }],
  nextMove: "Thank them, then pair on the first fix.",
  nextWhen: "Right after this meeting",
} });
