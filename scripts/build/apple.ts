import { existsSync } from "node:fs";
import { mkdir, rm } from "node:fs/promises";
import { resolve } from "node:path";
import { verifyAppCommands } from "../release/app-commands";
const env: Record<string, string | undefined> = {
  ...process.env,
  PATH: "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:" + process.env.PATH,
};
async function run(cmd: string[]) {
  const child = Bun.spawn(cmd, { stdout: "inherit", stderr: "inherit", env });
  if (await child.exited) throw new Error(`Failed: ${cmd[0]}`);
}
const derived = process.env.HITSLOP_DERIVED_DATA ?? resolve(".hitslop/apple-build");
// XcodeGen validates every target, including iOS resources unused by the Mac scheme.
for (const name of ["StarterTemplates", "Fixtures"])
  await mkdir(`generated/ios/${name}`, { recursive: true });
await run(["xcodegen", "generate", "--spec", "apps/apple/project.yml"]);
await run([
  "xcodebuild",
  "-jobs",
  "4",
  "-skipPackagePluginValidation",
  "-quiet",
  "-project",
  "apps/apple/hitSlop.xcodeproj",
  "-scheme",
  "hitSlop-macOS",
  "-configuration",
  "Debug",
  "-destination",
  "platform=macOS",
  "-derivedDataPath",
  derived,
  "CODE_SIGNING_ALLOWED=NO",
  "build",
]);
// CI compiles the app targets only; release qualification runs the app itself.
if (process.argv.includes("--compile-only")) process.exit(0);
// Validate Xcode Run's product before it is copied for the artifact check.
await verifyAppCommands(resolve(derived, "Build/Products/Debug/hitSlop.app"));
await mkdir("generated/app", { recursive: true });
const artifact = resolve("generated/app/hitSlop.app");
// This is a disposable build artifact. Never merge old signed resources or
// read-only starter packages into the next app.
if (existsSync(artifact)) {
  await run(["/bin/chmod", "-R", "u+w", artifact]);
  await rm(artifact, { recursive: true });
}
await run([
  "/usr/bin/ditto",
  resolve(derived, "Build/Products/Debug/hitSlop.app"),
  artifact,
]);
await run([
  process.execPath,
  "scripts/release/artifact.ts",
  "--unsigned",
  resolve("generated/app/hitSlop.app"),
]);
// A development build never claims .slop in Finder; open it explicitly (`open -a`).
const lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
for (const app of [resolve(derived, "Build/Products/Debug/hitSlop.app"), artifact])
  await Bun.spawn([lsregister, "-u", app], { stdout: "ignore", stderr: "ignore" }).exited;
