import { buildSkills } from "../../packages/hitslop/src/cli/skills-build";

console.log(`Built ${(await buildSkills()).length} skill files`);
