/** Type-checks every active template in one compiler program, with the starter's
 * self-contained tsconfig. Discovery keeps archived examples out. */
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { exec } from "../../packages/hitslop/src/cli/process";
import { repository } from "../lib/artifacts";
import { discoverTemplates } from "./discover";

const templates = await discoverTemplates();
const standard = await Bun.file(join(repository, "packages/hitslop/templates/checklist/tsconfig.json")).json();
const temporary = await mkdtemp(join(repository, ".build-test-check-"));
try {
  const config = join(temporary, "tsconfig.json");
  await writeFile(
    config,
    JSON.stringify({
      compilerOptions: standard.compilerOptions,
      include: templates.flatMap(({ source }) => [join(source, "**/*.svelte"), join(source, "**/*.ts")]),
      exclude: [join(repository, "**/node_modules/**"), join(repository, "**/dist/**")],
    }),
  );
  if (templates.length) {
    const command = [process.execPath, "node_modules/svelte-check/bin/svelte-check", "--workspace", join(repository, "examples/slops"), "--tsconfig", config];
    const { code } = await exec(command, { cwd: repository, env: process.env, inherit: ["stdout", "stderr"] });
    if (code) process.exitCode = code;
  }
} finally {
  await rm(temporary, { recursive: true, force: true });
}
