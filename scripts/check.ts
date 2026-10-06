import { repository } from "./runtime-artifacts";
import { discoverTemplates } from "./templates";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
const env = { ...process.env, PATH: "/opt/homebrew/bin:/usr/bin:/bin:" + process.env.PATH };
async function run(cmd: string[]) {
  const p = Bun.spawn(cmd, { stdout: "inherit", stderr: "inherit", env });
  if (await p.exited) throw new Error(`Check failed: ${cmd.join(" ")}`);
}
await run([process.execPath, "scripts/generate.ts", "--check"]);
await run([process.execPath, "scripts/skills.ts", "--check"]);
await run([process.execPath, "node_modules/typescript/bin/tsc", "-p", "tsconfig.json"]);
const templates = await discoverTemplates();
// One compiler program for every template, with the starter's self-contained tsconfig.
// Discovery keeps archived examples out.
const standard = await Bun.file(join(repository, "packages/cli/templates/checklist/tsconfig.json")).json();
const temporary = await mkdtemp(join(repository, ".build-test-check-"));
try {
  const config = join(temporary, "tsconfig.json");
  await writeFile(
    config,
    JSON.stringify({
      compilerOptions: standard.compilerOptions,
      include: templates.flatMap(({ source }) => [
        join(source, "**/*.svelte"),
        join(source, "**/*.ts"),
      ]),
      exclude: [join(repository, "**/node_modules/**"), join(repository, "**/dist/**")],
    }),
  );
  if (templates.length)
    await run([
      process.execPath,
      "node_modules/svelte-check/bin/svelte-check",
      "--workspace",
      join(repository, "examples/slops"),
      "--tsconfig",
      config,
    ]);
} finally {
  await rm(temporary, { recursive: true, force: true });
}
