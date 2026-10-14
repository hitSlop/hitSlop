/** Type-checks active templates and authored test fixtures in one compiler program, with
 * the shared authoring options, then the SDK's Svelte components with the options the
 * repository's own TypeScript is checked with. Discovery keeps archived examples out. */
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { exec } from "../../packages/hitslop/src/cli/process";
import { repository } from "../lib/artifacts";
import { discoverTemplates } from "./discover";

const templates = await discoverTemplates();
const fixtureSources = [...new Bun.Glob("tests/apps/*/slop.ts").scanSync(repository)].map(file => join(repository, dirname(file)));
const standard = await Bun.file(join(repository, "packages/hitslop/tsconfig.slop.json")).json();
const sdk = join(repository, "packages/hitslop/src/sdk");
const temporary = await mkdtemp(join(repository, ".build-test-check-"));
/** svelte-check over `workspace`, with `config` written beside the checkout's node_modules. */
async function svelteCheck(workspace: string, name: string, config: object) {
  const path = join(temporary, name);
  await writeFile(path, JSON.stringify(config));
  const command = [process.execPath, "node_modules/svelte-check/bin/svelte-check", "--workspace", workspace, "--tsconfig", path];
  const { code } = await exec(command, { cwd: repository, env: process.env, inherit: ["stdout", "stderr"] });
  if (code) process.exitCode = code;
}
try {
  if (templates.length || fixtureSources.length)
    await svelteCheck(join(repository, "slops"), "tsconfig.json", {
      compilerOptions: standard.compilerOptions,
      include: [...templates.map(({ source }) => source), ...fixtureSources].flatMap(source => [join(source, "**/*.svelte"), join(source, "**/*.ts")]),
      exclude: [join(repository, "**/node_modules/**"), join(repository, "**/dist/**")],
    });
  await svelteCheck(sdk, "sdk.json", { extends: join(repository, "tsconfig.json"), include: [join(sdk, "**/*.svelte")] });
} finally {
  await rm(temporary, { recursive: true, force: true });
}
