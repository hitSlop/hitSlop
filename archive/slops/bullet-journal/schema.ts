import { defineDocument, s, type Value } from "@hitslop/document";
import { signifierTypes } from "./signifiers";

const schema = defineDocument({
  date: s.text(),
  monthTitle: s.text(),
  dailyPage: s.text(),
  monthlyPage: s.text(),
  entries: s.list(s.object({
    type: s.enum(signifierTypes),
    star: s.boolean(),
    text: s.text(),
  })),
  monthlyLog: s.list(s.object({
    day: s.integer({ min: 1, max: 31 }),
    weekday: s.text(),
    text: s.text(),
  })),
});

export type BulletJournal = Value<typeof schema.fields.node>;
export type JournalEntry = BulletJournal["entries"][number];
export default schema;
