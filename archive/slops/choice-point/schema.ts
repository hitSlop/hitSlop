import { defineDocument, s, type Value } from "@hitslop/document";

const item = s.object({ text: s.text() });

const schema = defineDocument({
  date: s.text(),
  situation: s.text(),
  hooks: s.list(item),
  awayMoves: s.list(item),
  towardsMoves: s.list(item),
  helpers: s.list(item),
  nextMove: s.text(),
  nextWhen: s.text(),
  nextDone: s.boolean(),
});

export type ChoicePoint = Value<typeof schema.fields.node>;
export type ChoiceItem = ChoicePoint["hooks"][number];
export default schema;
