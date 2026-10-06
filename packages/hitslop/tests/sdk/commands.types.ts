import { defineDocument, s, Type } from "../../src/sdk/schema";
const doc = defineDocument({ title: s.text(), count: s.counter() });
const rename = doc.command({ description: "Rename", args: Type.Object({ title: Type.String() }, { additionalProperties: false }), run({ tx, current }, { title }) { tx.fields.title.set(title); return current.count; } });
function callers() {
  const result: Promise<number> = rename({ title: "Next" });
  // @ts-expect-error Required command arguments cannot be omitted.
  rename();
  // @ts-expect-error Argument inference comes from TypeBox.
  rename({ title: 7 });
  // @ts-expect-error Unknown fields are not arguments.
  rename({ title: "Next", other: true });
  return result;
}
