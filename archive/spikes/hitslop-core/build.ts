import { copyFile, mkdir, rm } from "node:fs/promises";
import { join } from "node:path";
const root = import.meta.dir;
const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH}` };
async function run(command: string[]) {
  const p = Bun.spawn(command, { cwd: root, env, stdout: "inherit", stderr: "inherit" });
  if (await p.exited !== 0) throw new Error(`Failed: ${command.join(" ")}`);
}
await run(["bun", "generate.ts", "--check"]);
await run(["cargo", "build", "--locked", "--release", "-p", "hitslop-core-native-spike", "--features", "cli"]);
await run(["target/release/uniffi-bindgen", "generate", "--library", "target/release/libhitslop_core_native_spike.dylib", "--language", "swift", "--config", "native/uniffi.toml", "--out-dir", "dist/generated-swift"]);
await mkdir(join(root, "dist/bindings"), { recursive: true });
await mkdir(join(root, "dist/include"), { recursive: true });
await copyFile(join(root, "dist/generated-swift/HitSlopCoreBinding.swift"), join(root, "dist/bindings/HitSlopCoreBinding.swift"));
await copyFile(join(root, "dist/generated-swift/HitSlopCoreSpikeFFI.h"), join(root, "dist/include/HitSlopCoreSpikeFFI.h"));
await copyFile(join(root, "dist/generated-swift/HitSlopCoreSpikeFFI.modulemap"), join(root, "dist/include/module.modulemap"));
// Only disposable build output is replaced. No app resources or release artifacts.
await rm(join(root, "dist/HitSlopCoreSpikeFFI.xcframework"), { recursive: true, force: true });
await run(["/usr/bin/xcodebuild", "-create-xcframework", "-library", "target/release/libhitslop_core_native_spike.a", "-headers", "dist/include", "-output", "dist/HitSlopCoreSpikeFFI.xcframework"]);
await run(["/usr/bin/swift", "build", "-c", "release"]);
await run(["cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "hitslop-core-wasm-spike"]);
const bindgen = join(root, "dist/tools/bin/wasm-bindgen");
if (!await Bun.file(bindgen).exists()) throw new Error("Install the pinned wasm-bindgen-cli using the README bootstrap command");
await run([bindgen, "--target", "web", "--out-dir", "dist/wasm", "target/wasm32-unknown-unknown/release/hitslop_core_wasm_spike.wasm"]);

