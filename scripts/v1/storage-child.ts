import { writeFileSync } from "node:fs";
import { Document } from "../../packages/document/src/document";
import { SQLiteStore } from "../../packages/document/test-support/sqlite";
import { crashSchema as checklist, crashInitial as initial } from "./crash-matrix";
const [root, phase, marker] = process.argv.slice(2) as [string, string, string];
const io = await SQLiteStore.open(root, (at) => {
  if (at === phase) {
    writeFileSync(marker, at);
    process.kill(process.pid, "SIGSTOP");
  }
});
const doc = await Document.open(checklist, io, initial);
if (phase === "hold") {
  writeFileSync(marker, "held");
  process.kill(process.pid, "SIGSTOP");
}
if (phase.startsWith("append:")) {
  doc.fields.title.replace("Crash edit");
  await doc.flush();
} else await doc.compact();
await doc.close();
