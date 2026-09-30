import { writeSkills } from "@crustjs/skills";
import { fileURLToPath } from "node:url";
import { app, cliVersion, skillExtras, skillName } from "./app";

const packagedSkillsDirectory = fileURLToPath(
  new URL("../.crust/root/skills", import.meta.url),
);

export async function buildSkills(outDir = packagedSkillsDirectory) {
  return writeSkills({ app, outDir, name: skillName, version: cliVersion, extras: skillExtras });
}
