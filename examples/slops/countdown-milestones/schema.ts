import { defineDocument, s, type Value } from "@hitslop/document";

const schema = defineDocument({
  title: s.text(),
  createdAt: s.string(),
  targetDate: s.string(),
  targetTime: s.string(),
  milestones: s.list(s.object({
    title: s.text(),
    done: s.boolean(),
  })),
});

export type Countdown = Value<typeof schema.descriptor>;
export type Milestone = Countdown["milestones"][number];
export default schema;
