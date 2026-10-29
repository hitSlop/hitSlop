import { buildSkills } from "../../packages/tinys/src/cli/skills-build";

console.log(`Built ${(await buildSkills()).length} skill files`);
