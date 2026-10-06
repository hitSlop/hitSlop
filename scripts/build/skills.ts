import { buildSkills } from "../../packages/cli/src/skills-build";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

if (process.argv.includes("--check")) {
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-skills-check-"));
  try {
    console.log(`Validated ${(await buildSkills(join(temporary, "skills"))).length} skill files`);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
} else console.log(`Built ${(await buildSkills()).length} skill files`);
