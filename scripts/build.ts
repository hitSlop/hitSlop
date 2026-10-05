import { repository } from "./runtime-artifacts";
import { buildCoreNative, buildCoreWasm, buildEngine } from "./core-build";
import { buildSkills } from "../packages/cli/src/skills-build";
import { generateContracts } from "./generate";
import { buildShell } from "./runtime";
import { join } from "node:path";
import { copyFile } from "node:fs/promises";
const started = performance.now();
console.log("Building contracts, runtime, file engine, skills, and native helper");
await generateContracts();
await buildCoreWasm();
await buildEngine();
await buildCoreNative();
await buildShell();
await buildSkills();
const build = Bun.spawn(
  [
    "swift",
    "build",
    "--package-path",
    join(repository, "apps/apple/Packages/HitSlopApple"),
    "--product",
    "hitslop-native",
  ],
  { stdout: "inherit", stderr: "inherit" },
);
if (await build.exited) throw new Error("Native build failed");
// An explicit debug helper selects this sibling as the same native deployment.
await copyFile(join(repository, "target/release/slop-engine"), join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/slop-engine"));
console.log(
  `Built native development resources in ${((performance.now() - started) / 1000).toFixed(1)}s`,
);
