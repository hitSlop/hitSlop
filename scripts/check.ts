import { discoverTemplates, repository, type TemplateSource } from "./templates";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { isDeepStrictEqual } from "node:util";
const env = { ...process.env, PATH: "/opt/homebrew/bin:/usr/bin:/bin:" + process.env.PATH };
async function run(cmd: string[]) {
  const p = Bun.spawn(cmd, { stdout: "inherit", stderr: "inherit", env });
  if (await p.exited) throw new Error(`Check failed: ${cmd.join(" ")}`);
}
await run([process.execPath, "scripts/generate.ts", "--check"]);
await run([process.execPath, "scripts/skills.ts", "--check"]);
await run([process.execPath, "node_modules/typescript/bin/tsc", "-p", "tsconfig.json"]);
const templates = await discoverTemplates();
// Share the standard compiler program; preserve project-specific settings when
// a template opts into them. Discovery keeps archived examples out.
const shared: TemplateSource[] = [],
  custom: TemplateSource[] = [];
const standard = {
  extends: "../../../tsconfig.json",
  include: ["./**/*.svelte", "./**/*.ts"],
  exclude: ["dist", "node_modules"],
};
for (const template of templates) {
  const config = await Bun.file(join(template.source, "tsconfig.json")).text();
  let settings: unknown;
  try {
    settings = JSON.parse(config);
  } catch {
    /* svelte-check handles JSONC and diagnostics. */
  }
  (isDeepStrictEqual(settings, standard) ? shared : custom).push(template);
}
const temporary = await mkdtemp(join(repository, ".build-test-check-"));
try {
  const config = join(temporary, "tsconfig.json");
  await writeFile(
    config,
    JSON.stringify({
      extends: join(repository, "tsconfig.json"),
      include: shared.flatMap(({ source }) => [
        join(source, "**/*.svelte"),
        join(source, "**/*.ts"),
      ]),
      exclude: [join(repository, "**/node_modules/**"), join(repository, "**/dist/**")],
    }),
  );
  if (shared.length)
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
for (const { source } of custom) {
  await run([
    process.execPath,
    "node_modules/svelte-check/bin/svelte-check",
    "--workspace",
    source,
    "--tsconfig",
    "tsconfig.json",
  ]);
}
