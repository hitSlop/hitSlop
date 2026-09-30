import { copyFile, mkdir, rm } from "node:fs/promises";
import { resolve, join } from "node:path";
import { existsSync } from "node:fs";

const root = resolve(import.meta.dir, "..");
const env = {
  ...process.env,
  PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH}`,
};
async function run(command: string[]) {
  const child = Bun.spawn(command, { cwd: root, env, stdout: "inherit", stderr: "inherit" });
  if (await child.exited) throw new Error(`Core build failed: ${command.join(" ")}`);
}

/** Browser/dev/test binding. This entry point works on Linux without Xcode. */
export async function buildCoreWasm() {
  const local = join(root, "generated/core-tools/bin/wasm-bindgen");
  const bindgen = process.env.HITSLOP_WASM_BINDGEN ?? (existsSync(local) ? local : "wasm-bindgen");
  const version = Bun.spawn([bindgen, "--version"], { env, stdout: "pipe", stderr: "inherit" });
  const actual = (await new Response(version.stdout).text()).trim();
  if ((await version.exited) || actual !== "wasm-bindgen 0.2.127") {
    throw new Error(
      "Install matching tooling: cargo install wasm-bindgen-cli --version 0.2.127 --locked --root generated/core-tools",
    );
  }
  await run([
    "cargo",
    "build",
    "--locked",
    "--release",
    "--target",
    "wasm32-unknown-unknown",
    "-p",
    "hitslop-core-wasm",
  ]);
  await run([
    bindgen,
    "--target",
    "web",
    "--out-dir",
    "generated/core/wasm",
    "target/wasm32-unknown-unknown/release/hitslop_core_wasm.wasm",
  ]);
  // The CLI compares this with the native helper's without compiling the WASM.
  const output = join(root, "generated/core/wasm");
  const core = await import(join(output, "hitslop_core_wasm.js"));
  core.initSync({ module: await Bun.file(join(output, "hitslop_core_wasm_bg.wasm")).bytes() });
  await Bun.write(join(output, "BUILD_ID"), core.coreBuildId() + "\n");
}

/** Native bindings are generated from the same locked core as the WASM binding. */
export async function buildCoreNative() {
  if (process.platform !== "darwin") throw new Error("Native core packaging requires macOS");
  const generated = join(root, "apps/apple/Packages/HitSlopApple/Generated");
  const binding = join(generated, "HitSlopCoreBinding");
  const headers = join(generated, "include");
  const framework = join(generated, "HitSlopCoreFFI.xcframework");
  await run([
    "cargo",
    "build",
    "--locked",
    "--release",
    "-p",
    "hitslop-core-ffi",
    "--features",
    "cli",
  ]);
  await run([
    "target/release/uniffi-bindgen",
    "generate",
    "--library",
    "target/release/libhitslop_core_ffi.dylib",
    "--language",
    "swift",
    "--config",
    "crates/hitslop-core-ffi/uniffi.toml",
    "--out-dir",
    "generated/core/swift",
  ]);
  await mkdir(binding, { recursive: true });
  await mkdir(headers, { recursive: true });
  await copyFile(
    join(root, "generated/core/swift/HitSlopCoreBinding.swift"),
    join(binding, "HitSlopCoreBinding.swift"),
  );
  await copyFile(
    join(root, "generated/core/swift/HitSlopCoreFFI.h"),
    join(headers, "HitSlopCoreFFI.h"),
  );
  await copyFile(
    join(root, "generated/core/swift/HitSlopCoreFFI.modulemap"),
    join(headers, "module.modulemap"),
  );
  // SwiftPM tests use the machine architecture, while release archives are arm64.
  // A universal macOS library supports both without depending on the CI runner CPU.
  const targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
  await run(["rustup", "target", "add", ...targets]);
  for (const target of targets) {
    await run([
      "cargo",
      "build",
      "--locked",
      "--release",
      "--target",
      target,
      "-p",
      "hitslop-core-ffi",
    ]);
  }
  const library = join(generated, "libhitslop_core_ffi.a");
  await run([
    "lipo",
    "-create",
    ...targets.map((target) => join(root, "target", target, "release/libhitslop_core_ffi.a")),
    "-output",
    library,
  ]);
  // Replace disposable artifacts only; immutable runtime releases are never outputs.
  await rm(framework, { recursive: true, force: true });
  await run([
    "xcodebuild",
    "-create-xcframework",
    "-library",
    library,
    "-headers",
    headers,
    "-output",
    framework,
  ]);
}

if (import.meta.main) {
  if (process.argv.includes("--wasm")) await buildCoreWasm();
  else if (process.argv.includes("--native")) await buildCoreNative();
  else {
    await buildCoreWasm();
    await buildCoreNative();
  }
}
