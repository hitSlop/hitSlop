// Measures how full-history snapshots grow under realistic long editing sessions.
// Diagnostic only: prints sizes against the host storage capacity; asserts nothing.
import { Document } from "../../packages/document/src/document";
import { MemoryStore } from "../../packages/document/src/memory";
import { defineDocument, s } from "../../packages/document/src/schema";

const capacity = 32 * 1024 * 1024;
const definition = defineDocument({
  body: s.text(),
  tasks: s.list(s.object({ text: s.text(), done: s.boolean() })),
  taps: s.counter(),
});
const doc = await Document.open(definition, new MemoryStore(), { body: "", tasks: [], taps: 0 });
const size = () => doc.exportSnapshot().length;
const rows: [string, number, number][] = [];
const record = (label: string, edits: number) => rows.push([label, edits, size()]);

// Typing: one keystroke per edit, with occasional backspaces, like a journal.
let edits = 0;
for (let day = 0; day < 200; day++) {
  for (const char of `Day ${day}: wrote about the plan and what changed.\n`) {
    doc.fields.body.splice(doc.current.body.length, 0, char);
    if (++edits % 17 === 0) doc.fields.body.splice(doc.current.body.length - 1, 1, "");
  }
  if (day === 49 || day === 199) record(`typing (${day + 1} entries)`, edits);
}
// Checklist churn: add, check, reorder and delete rows.
for (let i = 0; i < 5000; i++) {
  const { id } = doc.fields.tasks.insert({ text: `Task ${i}`, done: false });
  doc.fields.tasks.item(id).done.set(true);
  const first = doc.current.tasks[0]!.$id;
  if (doc.current.tasks.length > 1) doc.fields.tasks.move(first, { after: doc.current.tasks.at(-1)!.$id });
  if (doc.current.tasks.length > 30) doc.fields.tasks.remove(doc.current.tasks[0]!.$id);
  edits += 4;
  if (i === 999 || i === 4999) record(`checklist churn (${i + 1} tasks)`, edits);
}
// Counter taps.
for (let i = 0; i < 100_000; i++) doc.fields.taps.increment();
record("counter (100k taps)", (edits += 100_000));
await doc.close();

console.log("scenario".padEnd(32), "edits".padStart(8), "snapshot".padStart(12), "of capacity".padStart(12));
for (const [label, count, bytes] of rows)
  console.log(
    label.padEnd(32),
    String(count).padStart(8),
    `${(bytes / 1024).toFixed(0)} KiB`.padStart(12),
    `${((bytes / capacity) * 100).toFixed(2)}%`.padStart(12),
  );

// Large documents: per-save snapshot export and edit latency (row edits resolve IDs
// through the projected snapshot; change() stages on a fork of the whole document).
const rowsDefinition = defineDocument({ rows: s.list(s.object({ text: s.text(), done: s.boolean() })) });
const time = (work: () => unknown) => {
  work();
  const start = performance.now();
  for (let i = 0; i < 5; i++) work();
  return ((performance.now() - start) / 5).toFixed(1);
};
console.log("\nrows".padEnd(33), "snapshot".padStart(9), "export".padStart(9), "row edit".padStart(9), "change()".padStart(9));
for (const count of [5_000, 40_000]) {
  const large: any = await Document.open(rowsDefinition, new MemoryStore(), {
    rows: Array.from({ length: count }, (_, i) => ({ text: `${i} ${"lorem ipsum ".repeat(16)}`, done: false })),
  });
  const id = large.current.rows[count - 1].$id;
  console.log(
    String(count).padEnd(32),
    `${(large.exportSnapshot().length / 1024 / 1024).toFixed(1)} MiB`.padStart(9),
    `${time(() => large.exportSnapshot())} ms`.padStart(9),
    `${time(() => large.fields.rows.item(id).done.set(true))} ms`.padStart(9),
    `${time(() => large.change((tx: any) => tx.fields.rows.item(id).done.set(false)))} ms`.padStart(9),
  );
  await large.close();
}
