// A fresh process evaluates a project's `slop.ts` (author code) once:
//   stage-worker SOURCE --check                         checks it as a build does, writing nothing
//   stage-worker SOURCE STAGE [--compile] [--deps FILE]  writes the stage (`app.json`; with
//     --compile, the compiled app, its assets and artwork) and, with --deps, slop.ts's local
//     imports: dev's watch list.
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { loadProject, metadataFiles, normalizeApp, stageProjectInBun } from "./build";
import { localImports } from "./imports";
import { discoverEntry } from "./entry";

const [source, ...args] = process.argv.slice(2) as [string, ...string[]];
const option = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
if (args.includes("--check")) {
  await discoverEntry(source);
  await normalizeApp(source, await loadProject(source));
}
else {
  const compile = args.includes("--compile") ? (await import("./vite")).compileAppWithVite : undefined;
  await stageProjectInBun(source, args[0]!, compile);
  const deps = option("--deps");
  if (deps) await writeFile(deps, JSON.stringify(await localImports(metadataFiles.map((file) => join(source, file)))));
}
