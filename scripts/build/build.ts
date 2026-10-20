/** The complete development build and individual verification preparations. Outputs
 * retain their timestamps when their content is unchanged. */
import { buildCoreNative, buildCoreWasm, buildEngine } from "./core";
import { buildSkills } from "../../packages/hitslop/src/cli/skills-build";
import { generateContracts } from "./generate";
import { buildShell } from "./shell";
import { buildBrowser } from "./browser";
import { prerequisites, type Build } from "./preparation";

/** One output only; callers prepare its prerequisites through the shared graph. */
export async function prepareOutput(name: Build) {
  switch (name) {
    case "engine": return buildEngine();
    case "shell": return buildShell();
    case "native": return buildCoreNative();
    case "sdk": return buildCoreWasm();
    case "browser": return buildBrowser();
    case "sharing": return (await import("./sharing")).buildSharing();
    case "templates": return (await import("../templates/build")).buildTemplates();
    case "packages": return (await import("./packages")).packPackages(true);
    case "app": await import("./apple"); return;
  }
}

/** Prepare a set of outputs and their prerequisites without repeating shared work. */
export async function prepareBuild(names: Build[]) {
  const done = new Set<Build>();
  async function prepare(name: Build) {
    if (done.has(name)) return;
    for (const dependency of prerequisites[name]) await prepare(dependency);
    await prepareOutput(name);
    done.add(name);
  }
  for (const name of names) await prepare(name);
}

export async function buildNative() {
  const started = performance.now();
  console.log("Building contracts, runtime, file engine, and skills");
  await generateContracts();
  await prepareBuild(["sdk", "native", "shell"]);
  await buildSkills();
  console.log(`Built native development resources in ${((performance.now() - started) / 1000).toFixed(1)}s`);
}

if (import.meta.main) {
  const option = process.argv.includes("--output") ? "--output" : "--prepare";
  if (process.argv.includes(option)) {
    const names = process.argv[process.argv.indexOf(option) + 1]?.split(",");
    if (!names?.length || names.some(name => !Object.hasOwn(prerequisites, name)))
      throw new Error(`Unknown preparation: ${names}`);
    if (option === "--output") for (const name of names) await prepareOutput(name as Build);
    else await prepareBuild(names as Build[]);
  } else await buildNative();
}
