import { join } from "node:path";
import { writeFile } from "node:fs/promises";
import { buildProjectInBun } from "./build";
import { localImports } from "./imports";

// A fresh process evaluates authored metadata; its imports are also the watch contract.
const [source, output, dependencies] = process.argv.slice(2) as [string, string, string];
const entries = ["schema.ts", "initial.ts", "theme.ts"].map((file) => join(source, file));
await writeFile(dependencies, JSON.stringify([join(source, "manifest.json"), ...(await localImports(entries))]));
await buildProjectInBun(source, output);
