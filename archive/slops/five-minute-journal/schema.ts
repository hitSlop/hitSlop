import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  date: s.string(),
  quote: s.text(),
  quoteAuthor: s.text(),
  morningDone: s.boolean(),
  gratitude1: s.text(),
  gratitude2: s.text(),
  gratitude3: s.text(),
  intention1: s.text(),
  intention2: s.text(),
  intention3: s.text(),
  affirmation: s.text(),
  eveningDone: s.boolean(),
  highlight1: s.text(),
  highlight2: s.text(),
  highlight3: s.text(),
  lesson: s.text(),
});

export type FiveMinuteJournal = Value<typeof schema.fields.node>;
export default schema;
