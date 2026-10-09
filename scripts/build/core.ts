/** The shared document core, as each host consumes it: the WASM binding (SDK and Bun
 * tests), the CLI's file engine, and the app's Swift binding and library. Every host build
 * shares one Cargo graph, and an output whose bytes did not change is not rewritten, so
 * nothing downstream rebuilds. */
import { publishFolder, repository, writeIfChanged } from "../lib/artifacts";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { homedir, tmpdir } from "node:os";
import { existsSync } from "node:fs";
import { exec } from "../../packages/hitslop/src/cli/process";
import platforms from "./platforms.json";

/** Release binaries name dependency sources by a fixed prefix instead of this machine's
 * Cargo home, so none carries a local path, and builds of the same sources on different
 * machines match. Workspace crates are already named relative to the checkout. Every
 * release build passes the same flags, so all share one Cargo graph; the Xcode embed and
 * the Engines workflow get them from `bun scripts/build/core.ts --rustflags`. */
export function releaseRustflags(): string[] {
  const cargoHome = process.env.CARGO_HOME ?? join(homedir(), ".cargo");
  return [
    `--remap-path-prefix=${cargoHome}/registry/src=/cargo/registry/src`,
    `--remap-path-prefix=${cargoHome}/git/checkouts=/cargo/git/checkouts`,
  ];
}
const env = {
  ...process.env,
  PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH}`,
  // Encoded, so a Cargo home with spaces stays one argument.
  CARGO_ENCODED_RUSTFLAGS: releaseRustflags().join("\x1f"),
};
/** The Cargo profile every core build uses: `release` while developing, `dist` (fat LTO) for
 * what ships, chosen by `HITSLOP_CARGO_PROFILE` (the release gate, compatibility capture,
 * the release workflow). Read at each build, so an entry point can choose it at run time. */
export function cargoProfile(): string {
  return process.env.HITSLOP_CARGO_PROFILE || "release";
}
/** A build output of `cargoProfile()`, for the host or for `target`. */
export function cargoOutput(file: string, target?: string): string {
  return join(repository, "target", ...(target ? [target] : []), cargoProfile(), file);
}
async function run(command: string[]) {
  const { code } = await exec(command, { cwd: repository, env, inherit: ["stdout", "stderr"] });
  if (code) throw new Error(`Core build failed: ${command.join(" ")}`);
}
/** A pinned generator: `generated/core-tools/bin/<name>` (or `variable`), refused at any
 * other version. */
async function generator(name: string, version: string, variable: string, install: string, localPath = `generated/core-tools/bin/${name}`) {
  const local = join(repository, localPath);
  const binary = process.env[variable] ?? (existsSync(local) ? local : name);
  const { stdout, code } = await exec([binary, "--version"], { env }).catch(() => ({ stdout: "", code: 1 }));
  // Official Binaryen archives append a release tag; Homebrew omits it.
  const reported = stdout.trim().replace(/ \(version_\d+\)$/, "");
  if (code || reported !== `${name} ${version}`) throw new Error(`Install matching tooling: ${install}`);
  return binary;
}

/** Test binding: the SDK and shell tests run the core in Bun. This entry point works on
 * Linux without Xcode. */
export async function buildCoreWasm() {
  const bindgen = await generator(
    "wasm-bindgen",
    "0.2.129",
    "HITSLOP_WASM_BINDGEN",
    "cargo install wasm-bindgen-cli --version 0.2.129 --locked --root generated/core-tools",
  );
  const target = "wasm32-unknown-unknown";
  await run(["cargo", "build", "--locked", "--profile", cargoProfile(), "--target", target, "-p", "hitslop-core-wasm"]);
  // Bindings are a pure function of the module: an unchanged one keeps the folder as is.
  await publishFolder(join(repository, "generated/core/wasm"), (stage) =>
    run([bindgen, "--target", "web", "--out-dir", stage, cargoOutput("hitslop_core_wasm.wasm", target)]),
  );
}

/** SQLite and QuickJS compile C for wasm32; Apple's clang has no WebAssembly backend. */
export async function buildBrowserWasm() {
  const optimizer = await generator("wasm-opt", "version 132", "HITSLOP_WASM_OPT",
    "bash scripts/build/install-binaryen.sh (requires Binaryen 132; HITSLOP_WASM_OPT may select its wasm-opt)",
    "generated/core-tools/binaryen/bin/wasm-opt");
  const llvm = process.platform === "darwin" ? "/opt/homebrew/opt/llvm/bin/" : "";
  const browserEnv = {
    ...env,
    CC_wasm32_unknown_unknown: process.env.CC_wasm32_unknown_unknown || `${llvm}clang`,
    AR_wasm32_unknown_unknown: process.env.AR_wasm32_unknown_unknown || `${llvm}llvm-ar`,
  };
  const bindgen = await generator("wasm-bindgen", "0.2.129", "HITSLOP_WASM_BINDGEN",
    "cargo install wasm-bindgen-cli --version 0.2.129 --locked --root generated/core-tools");
  const target = "wasm32-unknown-unknown";
  for (const [feature, name] of [["browser", "core"], ["evaluator", "evaluator"]] as const) {
    const command = ["cargo", "build", "--locked", "--profile", "wasm", "--target", target, "-p", "hitslop-core-wasm", "--features", feature];
    const { code } = await exec(command, { cwd: repository, env: browserEnv, inherit: ["stdout", "stderr"] });
    if (code) throw new Error(`Browser ${name} build failed`);
    await publishFolder(join(repository, "generated/browser", name), async stage => {
      await run([bindgen, "--target", "web", "--out-dir", stage, join(repository, "target", target, "wasm/hitslop_core_wasm.wasm")]);
      await run([optimizer, "-Oz", "--enable-bulk-memory", "--enable-sign-ext", "--enable-nontrapping-float-to-int", "--enable-mutable-globals", "--enable-reference-types", "--enable-multivalue", join(stage, "hitslop_core_wasm_bg.wasm"), "-o", join(stage, "hitslop_core_wasm_bg.wasm")]);
    });
  }
}

/** Every platform a published CLI carries a file engine for, built by the engines workflow
 * (`.github/workflows/engines.yml`); a local build covers only this machine. */
export const enginePlatforms = platforms.map(item => item.platform);

/** The CLI's file engine (`cargoOutput("slop-engine")`), from the same locked core. On a Mac
 * it builds with the app's core library: alone, the engine's graph would differ (the
 * library's build dependencies add features), and each build would undo the other. */
export async function buildEngine() {
  const library = process.platform === "darwin" ? ["-p", "hitslop-core-ffi"] : [];
  await run(["cargo", "build", "--locked", "--profile", cargoProfile(), "-p", "slop-engine", "-p", "hitslop-runner", ...library]);
}

/** The app's core: the Swift binding and an arm64 XCFramework (the app ships for Apple
 * silicon only), built in the same Cargo graph as the engine. */
export async function buildCoreNative() {
  if (process.platform !== "darwin" || process.arch !== "arm64") throw new Error("The native core builds on an Apple silicon Mac");
  const bindgen = await generator(
    "uniffi-bindgen",
    "0.32.2",
    "HITSLOP_UNIFFI_BINDGEN",
    "cargo install uniffi --version 0.32.2 --features cli --locked --root generated/core-tools --bin uniffi-bindgen",
  );
  await buildEngine();
  const generated = join(repository, "apps/apple/Packages/HitSlopApple/Generated");
  const headers = join(generated, "include");
  const bindings = await mkdtemp(join(tmpdir(), "hitslop-bindings-"));
  let changed = false;
  try {
    await run([
      bindgen,
      "generate",
      "--library",
      cargoOutput("libhitslop_core_ffi.dylib"),
      "--language",
      "swift",
      "--out-dir",
      bindings,
    ]);
    // SwiftPM recompiles everything that imports a touched file, so only changes are written.
    for (const [from, to] of [
      ["HitSlopCoreBinding.swift", join(generated, "HitSlopCoreBinding/HitSlopCoreBinding.swift")],
      ["HitSlopCoreFFI.h", join(headers, "HitSlopCoreFFI.h")],
      ["HitSlopCoreFFI.modulemap", join(headers, "module.modulemap")],
    ] as const)
      changed = (await writeIfChanged(to, await readFile(join(bindings, from)))) || changed;
  } finally {
    await rm(bindings, { recursive: true, force: true });
  }
  const library = join(generated, "libhitslop_core_ffi.a");
  changed = (await writeIfChanged(library, await readFile(cargoOutput("libhitslop_core_ffi.a")))) || changed;
  // Replace disposable artifacts only; immutable runtime releases are never outputs.
  const framework = join(generated, "HitSlopCoreFFI.xcframework");
  if (!changed && existsSync(framework)) return;
  await rm(framework, { recursive: true, force: true });
  await run(["xcodebuild", "-create-xcframework", "-library", library, "-headers", headers, "-output", framework]);
}

if (import.meta.main) {
  if (process.argv.includes("--rustflags")) console.log(releaseRustflags().join(" "));
  else if (process.argv.includes("--wasm")) await buildCoreWasm();
  else if (process.argv.includes("--native")) await buildCoreNative();
  else if (process.argv.includes("--engine")) await buildEngine();
  else {
    await buildCoreWasm();
    await buildEngine();
    await buildCoreNative();
  }
}
