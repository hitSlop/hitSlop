import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/manifest-validation-spike");
const workspace = join(out, "workspace");
const target = join(out, "target");
const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin` };
async function run(args: string[], log: string) {
  const child = Bun.spawn(args, { cwd: workspace, env, stdout: Bun.file(log), stderr: "pipe" });
  const errors = await new Response(child.stderr).text();
  const status = await child.exited;
  await writeFile(log + ".stderr", errors);
  if (status) throw new Error(`${args.join(" ")}\n${errors}`);
}
for (const mode of process.argv.slice(2).length ? process.argv.slice(2) : ["baseline", "runtime", "compiled"]) {
  if (!["baseline", "runtime", "compiled"].includes(mode)) throw new Error(`Unknown mode: ${mode}`);
  const dir = join(out, mode);
  await mkdir(dir, { recursive: true });
  const features = mode === "baseline" ? [] : ["--features", `hitslop-core/manifest-${mode}`];
  const common = ["cargo", "build", "--release", "--locked", "--target-dir", target];
  let start = performance.now();
  console.log(`Building ${mode} native`);
  await run([...common, "-p", "hitslop-core", "-p", "hitslop-core-ffi", ...features], join(dir, "native-build.log"));
  const nativeBuildSeconds = (performance.now() - start) / 1000;
  await copyFile(join(target, "release/manifest-bench"), join(dir, "manifest-bench"));
  await copyFile(join(target, "release/libhitslop_core_ffi.dylib"), join(dir, "libhitslop_core_ffi.dylib"));
  console.log(`Building ${mode} WASM`);
  start = performance.now();
  await run([...common, "--target", "wasm32-unknown-unknown", "-p", "hitslop-core-wasm", ...features], join(dir, "wasm-build.log"));
  await run([join(root, "generated/core-tools/bin/wasm-bindgen"), "--target", "web", "--out-dir", join(dir, "wasm"), join(target, "wasm32-unknown-unknown/release/hitslop_core_wasm.wasm")], join(dir, "bindgen.log"));
  await run(["cargo", "tree", "--locked", "-p", "hitslop-core-wasm", "--target", "wasm32-unknown-unknown", "-e", "normal", "--prefix", "none", ...features], join(dir, "dependencies.txt"));
  await run(["cargo", "tree", "--locked", "-p", "hitslop-core-wasm", "--target", "wasm32-unknown-unknown", "-e", "features", ...features], join(dir, "features.txt"));
  await writeFile(join(dir, "build-times.json"), JSON.stringify({ nativeBuildSeconds, wasmBuildSeconds: (performance.now() - start) / 1000 }, null, 2));
  console.log(`Built ${mode}`);
}
await run(["swiftc", "-O", join(out, "PlatformContract.swift"), join(out, "main.swift"), "-o", join(out, "swift-probe")], join(out, "swift-build.log"));
