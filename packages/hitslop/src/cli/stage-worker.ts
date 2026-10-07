// Vite's build state is process-local; the declaration itself runs in the restricted child.
import { stageProjectInBun } from "./build";
const [source, stage] = process.argv.slice(2);
if (!source || !stage) throw new Error("stage-worker SOURCE STAGE");
console.log(JSON.stringify(await stageProjectInBun(source, stage)));
