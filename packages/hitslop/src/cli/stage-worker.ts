// Vite's build state is process-local; the declaration itself runs in the restricted child.
import { stageProjectInBun } from "./build";
import { writeFile } from "node:fs/promises";
const [source, stage, result] = process.argv.slice(2);
if (!source || !stage || !result) throw new Error("stage-worker SOURCE STAGE RESULT");
await writeFile(result, JSON.stringify(await stageProjectInBun(source, stage)));
