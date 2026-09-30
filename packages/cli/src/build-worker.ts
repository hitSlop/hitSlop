import { buildProjectInBun } from "./build";
import { compileAppWithVite } from "./vite";
await buildProjectInBun(process.argv[2]!, process.argv[3], compileAppWithVite);
