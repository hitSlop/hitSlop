import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  date: "Fri, Sep 25, 2026",
  situation: "Feedback landed hard in the team review.",
  hooks: [{ text: "“I should already know this”" }, { text: "Hot face, tight chest" }, { text: "Urge to shut down" }],
  awayMoves: [{ text: "Put off the revision" }, { text: "Get short with my teammates" }],
  towardsMoves: [{ text: "Thank them for catching it" }, { text: "Pair on the first fix" }],
  helpers: [{ text: "Craft" }, { text: "Humility" }, { text: "Three slow breaths" }, { text: "Name it: “here’s the not-good-enough story”" }],
  nextMove: "Thank them, then pair on the first fix.",
  nextWhen: "Right after this meeting",
  nextDone: false,
} satisfies Input<typeof schema.fields.node>;
