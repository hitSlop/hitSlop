import { mkdir, readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { buildSkills } from "../packages/cli/src/skills-build";
import { buildShell } from "./runtime";
await buildShell();
await buildSkills();
const output = resolve("generated/npm");
await mkdir(output, { recursive: true });
for (const name of ["schema", "document", "cli"]) {
  const directory = resolve("packages", name);
  const metadata = JSON.parse(await readFile(join(directory, "package.json"), "utf8"));
  for (const value of Object.values(metadata.dependencies ?? {}))
    if (String(value).startsWith("workspace:") || String(value).startsWith("file:"))
      throw new Error("Local dependency in published package");
  const child = Bun.spawn([process.execPath, "pm", "pack", "--destination", output], {
    cwd: directory,
    stdout: "inherit",
    stderr: "inherit",
  });
  if (await child.exited) throw new Error(`Cannot pack ${name}`);
}
