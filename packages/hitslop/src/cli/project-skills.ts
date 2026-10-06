import { installSkill, uninstallSkill, getSkillStatus, loadPackagedSkills } from "@crustjs/skills";
import { join } from "node:path";
import { existsSync } from "node:fs";

/** Preserve the node_modules path: Bun may swap the symlink's target on an upgrade. */
export async function projectSkills(argv: string[]): Promise<boolean> {
  if (!["skills", "skill"].includes(argv[0] ?? "") || argv.some(a => a === "--help" || a === "-h")) return false;
  const project = argv.includes("--scope=project") || argv.some((a, i) => a === "--scope" && argv[i + 1] === "project");
  if (!project) return false;
  const action = argv[1]?.startsWith("-") || !argv[1] ? "install" : argv[1];
  if (!["install", "repair", "uninstall"].includes(action)) throw new Error(`Unknown skills action: ${action}`);
  const remaining = argv.slice(action === argv[1] ? 2 : 1).filter((a, i, all) => a !== "--all" && a !== "--scope=project" && a !== "--scope" && all[i - 1] !== "--scope");
  if (remaining.length) throw new Error(`Unknown skills option: ${remaining[0]}`);
  const root = join(process.cwd(), "node_modules/hitslop/.crust/root/skills");
  if (!existsSync(root)) throw new Error("Run bun install in this project before linking its skills.");
  for (const skill of loadPackagedSkills(root)) {
    const options = { name: skill.name, sourceDir: skill.sourceDir, scope: "project" as const };
    if (action === "uninstall") await uninstallSkill(options);
    else if (action === "repair") {
      const { agents } = await getSkillStatus(options);
      const owned = agents.filter(a => a.status === "linked" || a.status === "dangling").map(a => a.agent);
      if (owned.length) await installSkill({ ...options, agents: owned });
    } else await installSkill(options);
  }
  console.log(`Project skills ${action === "uninstall" ? "removed" : "linked through node_modules/hitslop"}.`);
  return true;
}
