/** Device/simulator proof builds. The macOS build keeps its existing default graph. */
import { mkdtemp, cp, copyFile, readFile, rm, readdir, stat } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { buildEngine, buildCoreNative, cargoOutput, cargoProfile, releaseRustflags } from "./core";
import { buildShell } from "./shell";
import { publishFolder, repository } from "../lib/artifacts";
import { buildTemplates } from "../templates/build";
import { discoverTemplates } from "../templates/discover";
import type { AppDeclaration } from "../../packages/hitslop/src/sdk/slop";
import { exec } from "../../packages/hitslop/src/cli/process";

const env = {
  ...process.env,
  PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH}`,
  CARGO_ENCODED_RUSTFLAGS: releaseRustflags().join("\x1f"),
  IPHONEOS_DEPLOYMENT_TARGET: "26.0",
};
async function run(command: string[]) {
  const result = await exec(command, { cwd: repository, env, inherit: ["stdout", "stderr"] });
  if (result.code) throw new Error(`iOS preparation failed: ${command[0]}`);
}
export async function buildCoreIOS() {
  await buildEngine();
  await buildCoreNative();
  const targets = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];
  for (const target of targets) {
    await run(["cargo", "build", "--locked", "--profile", cargoProfile(), "-p", "hitslop-core-ffi", "--no-default-features", "--features", "mobile-capture", "--target", target]);
  }
  const generated = join(repository, "apps/apple/Packages/HitSlopApple/Generated");
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-ios-framework-"));
  try {
    const framework = join(temporary, "HitSlopCoreFFI.xcframework");
    const command = ["xcodebuild", "-create-xcframework"];
    for (const target of [undefined, ...targets]) command.push("-library", cargoOutput("libhitslop_core_ffi.a", target), "-headers", join(generated, "include"));
    await run([...command, "-output", framework]);
    await publishFolder(join(generated, "HitSlopCoreFFI.xcframework"), stage => cp(framework, stage, { recursive: true }));
  } finally { await rm(temporary, { recursive: true, force: true }); }
}

export async function prepareIOSFixtures() {
  const corpus = process.env.HITSLOP_COMPAT_ROOT || join(repository, ".hitslop/compat/current");
  const corpusEntries = await readdir(corpus, { withFileTypes: true }).catch(error => {
    if (error.code !== "ENOENT") throw error;
    throw new Error(`Compatibility corpus not found at ${corpus}. Run bun run compat:resolve to fetch the published corpora, or set HITSLOP_COMPAT_ROOT to the resolved corpus directory.`);
  });
  const temporary = await mkdtemp(join(tmpdir(), "hitslop-ios-fixture-"));
  try {
    const source = join(repository, "tests/fixtures/checklist/document");
    await cp(source, temporary, { recursive: true });
    const ui = join(temporary, "assets/ui.js");
    const { writeFile } = await import("node:fs/promises");
    await writeFile(ui, (await readFile(ui, "utf8")).replace("const doc = ctx.document;", `const doc = ctx.document;
      globalThis.iosAttachment = () => ctx.attachments.import(new File(['phone attachment'], 'phone.txt', {type:'text/plain'}), (tx, ref) => tx.fields.title.set(ref.id));`));
    const protocol = await exec([cargoOutput("slop-engine"), "--protocol"], { cwd: repository, env });
    if (protocol.code) throw new Error("Cannot read the engine protocol");
    const version = (JSON.parse(protocol.stdout) as { version: number }).version;
    const app: unknown = JSON.parse(await readFile(join(temporary, "input.json"), "utf8"));
    await publishFolder(join(repository, "generated/ios/Fixtures"), async stage => {
      const request = { method: "pack", stage: temporary, file: join(stage, "host-probe.slop"), app, renderViews: true };
      const packed = await exec([cargoOutput("slop-engine"), "--client-protocol", String(version)], { cwd: repository, env, stdin: JSON.stringify(request) });
      if (packed.code || !(JSON.parse(packed.stdout) as {ok?: boolean}).ok) throw new Error(`Cannot pack iOS fixture: ${packed.stdout} ${packed.stderr}`);
      for (const entry of corpusEntries) {
        if (!(await stat(join(corpus, entry.name))).isDirectory()) continue;
        const version = entry.name;
        const documents = join(corpus, version, "documents");
        for (const name of await readdir(documents)) if (name.endsWith(".slop")) await copyFile(join(documents, name), join(stage, `released-${version}-${name}`));
      }
    });
  } finally { await rm(temporary, { recursive: true, force: true }); }
}

/** Repository-owned starter metadata; ordinary template discovery stays non-executing. */
export async function iphoneStarterSlugs(): Promise<string[]> {
  const starters: string[] = [];
  for (const template of await discoverTemplates()) {
    const metadata = (await import(join(template.source, "slop.ts"))).default as AppDeclaration;
    if (!metadata.categories.includes("popular")) continue;
    const unsupported = metadata.capabilities?.filter(capability =>
      ["camera", "microphone", "speech"].includes(capability));
    if (unsupported?.length) {
      console.log(`iPhone starter excluded: ${template.slug} (requires ${unsupported.join(", ")})`);
      continue;
    }
    starters.push(template.slug);
  }
  console.log(`iPhone starters: ${starters.join(", ")}`);
  return starters;
}

if (import.meta.main) {
  const icons = join(repository, "apps/apple/App/Shared/Assets.xcassets/AppIcon.appiconset");
  await run(["swift", join(repository, "scripts/build/ios-icon.swift"), join(icons, "appicon-1024.png"), join(icons, "appicon-ios-1024.png")]);
  await buildCoreIOS();
  // The app bundles the page shell; a device build carries the current one.
  await buildShell();
  await prepareIOSFixtures();
  await buildTemplates(join(repository,"generated/ios/StarterTemplates"),await iphoneStarterSlugs());
  await run(["xcodegen", "generate", "--spec", "apps/apple/project.yml"]);
}
