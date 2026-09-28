import { defineDocument, s, type Value } from "@hitslop/document";

export const KINDS = ["focus", "meeting", "break", "personal"] as const;
export type Kind = (typeof KINDS)[number];

const schema = defineDocument({
  date: s.string(),
  priorities: s.list(s.object({
    text: s.text(),
    done: s.boolean(),
  })),
  blocks: s.list(s.object({
    start: s.string(),
    end: s.string(),
    title: s.text(),
    kind: s.enum(KINDS),
  })),
  notes: s.text(),
});

export type Planner = Value<typeof schema.fields.node>;
export type Block = Planner["blocks"][number];
export type Priority = Planner["priorities"][number];
export default schema;
