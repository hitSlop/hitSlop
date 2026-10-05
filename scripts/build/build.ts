/** `bun run build`: what native tests and the debug helper need, from the contracts to the
 * helper itself. A step whose inputs did not change leaves its outputs untouched, so a
 * repeated build recompiles nothing. */
import { repository, verifyShellCopies } from "../lib/artifacts";
import { buildCoreNative, buildCoreWasm, buildEngine } from "./core";
import { buildSkills } from "../../packages/cli/src/skills-build";
import { generateContracts } from "./generate";
import { buildShell } from "./shell";
import { exec } from "../../packages/cli/src/process";
import { join } from "node:path";
import { copyFile } from "node:fs/promises";

export async function buildNative() {
  const started = performance.now();
  console.log("Building contracts, runtime, file engine, skills, and native helper");
  await generateContracts();
  await buildCoreWasm();
  await buildEngine();
  await buildCoreNative();
  await buildShell();
  await buildSkills();
  const swift = ["swift", "build", "--package-path", join(repository, "apps/apple/Packages/HitSlopApple"), "--product", "hitslop-native"];
  if ((await exec(swift, { inherit: ["stdout", "stderr"] })).code) throw new Error("Native build failed");
  // An explicit debug helper selects this sibling as the same native deployment.
  await copyFile(join(repository, "target/release/slop-engine"), join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/slop-engine"));
  await verifyShellCopies();
  console.log(`Built native development resources in ${((performance.now() - started) / 1000).toFixed(1)}s`);
}

if (import.meta.main) await buildNative();
