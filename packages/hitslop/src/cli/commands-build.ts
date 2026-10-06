import { join } from "node:path";
import { mkdir, writeFile, realpath } from "node:fs/promises";
import { bindCommands, commandInfo } from "../sdk/commands";
import { CommandAssets, CommandMetadata } from "../schema/commands";
import { validate } from "../schema/validation";
import { exists } from "./fs";
import { localImports } from "./imports";

export async function buildCommands(source: string, definition: object, stage?: string) {
  source = await realpath(source);
  const entry = join(source, "commands.ts");
  if (!(await exists(entry, true))) return;
  for (const path of await localImports([entry])) {
    if (!(await realpath(path)).startsWith(source + "/")) throw new Error("Keep commands.ts imports inside the project");
    if (!/\.[cm]?[jt]s$/.test(path)) throw new Error("Commands import plain TypeScript or JavaScript modules");
  }
  const commands = await import(entry);
  bindCommands(commands, definition);
  const metadata = Object.fromEntries(Object.entries(commands).map(([name, command]) => {
    const { spec } = commandInfo(command)!;
    return [name, { description: spec.description, args: spec.args }];
  }));
  validate(CommandMetadata, metadata);
  const bundle = await Bun.build({
    entrypoints: ["hitslop:commands"], target: "browser", format: "iife", minify: true,
    plugins: [{ name: "commands-entry", setup(build) {
      build.onResolve({ filter: /^hitslop:commands$/ }, () => ({ path: "entry", namespace: "commands" }));
      build.onLoad({ filter: /.*/, namespace: "commands" }, () => ({ loader: "ts", contents: `import * as commands from ${JSON.stringify(entry)}; globalThis.__slopCommands = commands;` }));
    } }],
    sourcemap: "none",
  });
  if (!bundle.success) throw new Error(`Cannot bundle commands.ts: ${bundle.logs.join("\n")}`);
  const code = await bundle.outputs[0]!.text();
  if (code.includes(source)) throw new Error("The command bundle contains a local source path");
  if (stage) {
    await mkdir(join(stage, "assets/__commands"), { recursive: true });
    await writeFile(join(stage, "assets", CommandAssets.metadata), JSON.stringify(metadata));
    await writeFile(join(stage, "assets", CommandAssets.bundle), code);
  }
}
