import { defineDocument, s, type Value } from "@hitslop/document";
import { statuses } from "./board";

const schema = defineDocument({
  title: s.string({ maxLength: 24 }),
  rows: s.list(s.object({
    time: s.string({ maxLength: 5 }),
    text: s.string({ maxLength: 14 }),
    status: s.enum(statuses),
  })),
});

export type Board = Value<typeof schema.descriptor>;
export type Row = Board["rows"][number];
export default schema;
