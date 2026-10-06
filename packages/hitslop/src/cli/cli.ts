#!/usr/bin/env bun
import { delegate } from "./project";
try {
  const argv = await delegate(process.argv.slice(2));
  process.argv = [...process.argv.slice(0, 2), ...argv];
  const handled = ["skill", "skills"].includes(argv[0] ?? "") && await (await import("./project-skills")).projectSkills(argv);
  if (!handled) await import("./main");
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
