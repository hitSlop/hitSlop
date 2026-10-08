import { constants } from "node:fs";
import { access, copyFile, mkdir, readFile, readdir, readlink, lstat, symlink, writeFile, unlink } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { createHash } from "node:crypto";

export const root = resolve(import.meta.dir, "../../..");
export const out = join(root, "generated/boundary-simplification");
export const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin` };
export async function exists(path: string) { try { await access(path); return true; } catch { return false; } }
export async function command(args: string[], cwd = root) {
  const p = Bun.spawn(args, { cwd, env, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { stdout, stderr, code };
}
export async function copy(source: string, destination: string) {
  await mkdir(dirname(destination), { recursive: true });
  if ((await lstat(source)).isSymbolicLink()) await symlink(await readlink(source), destination);
  else await copyFile(source, destination, constants.COPYFILE_FICLONE);
}
export async function prepare() {
  if (await exists(join(out, "baseline.json"))) return;
  const inventory = await command(["/usr/bin/git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"]);
  if (inventory.code) throw new Error(inventory.stderr);
  const files = [...new Set(inventory.stdout.split("\0").filter(Boolean))].sort();
  const hashes: Record<string, string> = {};
  for (const file of files) {
    if (file.startsWith("archive/spikes/boundary-simplification/") || !(await exists(join(root, file)))) continue;
    await copy(join(root, file), join(out, "baseline", file));
    const info = await lstat(join(root, file));
    hashes[file] = createHash("sha256").update(info.isSymbolicLink() ? await readlink(join(root, file)) : await readFile(join(root, file))).digest("hex");
  }
  await writeFile(join(out, "baseline.json"), JSON.stringify({ date: new Date().toISOString(), head: (await command(["git", "rev-parse", "HEAD"])).stdout.trim(), digest: createHash("sha256").update(JSON.stringify(hashes)).digest("hex"), files: hashes, tools: await Promise.all([["bun", "--version"], ["rustc", "--version"], ["swift", "--version"]].map(a => command(a))) }, null, 2));
  // Build caches are shared only between sequential experiments, never with production.
  for (const name of ["target", "apps/apple/Packages/HitSlopApple/.build"]) {
    if (!(await exists(join(root, name)))) continue;
    const dest = join(out, "cache", name === "target" ? "rust" : "swift");
    await mkdir(dirname(dest), { recursive: true });
    const r = await command(["/bin/cp", "-cR", join(root, name), dest]);
    if (r.code) throw new Error(r.stderr);
  }
}
export async function workspace(name: string) {
  await prepare();
  const dest = join(out, name);
  if (await exists(dest)) throw new Error(`Workspace already exists: ${name}`);
  const baseline = JSON.parse(await readFile(join(out, "baseline.json"), "utf8"));
  for (const file of Object.keys(baseline.files)) await copy(join(out, "baseline", file), join(dest, file));
  for (const name of ["node_modules", "packages/cli/node_modules", "packages/document/node_modules", "packages/schema/node_modules", "examples/slops/node_modules", "apps/apple/Packages/HitSlopApple/Generated", "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/Resources", "packages/cli/shell", "generated/core", "generated/core-tools"]) {
    if (!(await exists(join(root, name)))) continue;
    await mkdir(dirname(join(dest, name)), { recursive: true });
    if (dest !== join(out, "A") && name.endsWith("node_modules")) {
      // Installed dependencies are read-only for this experiment; source and generated
      // bindings remain independent. No candidate modifies TS package implementations.
      await symlink(join(out, "A", name), join(dest, name));
      continue;
    }
    const r = await command(["/bin/cp", "-cR", join(root, name), join(dest, name)]);
    if (r.code) throw new Error(r.stderr);
  }
  await symlink(join(out, "cache/rust"), join(dest, "target"));
  await symlink(join(out, "cache/swift"), join(dest, "apps/apple/Packages/HitSlopApple/.build"));
  return dest;
}
export async function run(name: string, label: string, args: string[]) {
  await mkdir(join(out, "logs"), { recursive: true });
  const start = performance.now();
  const stdoutFile = join(out, "logs", `${name}-${label}.stdout`);
  const stderrFile = join(out, "logs", `${name}-${label}.stderr`);
  await writeFile(stdoutFile, "");
  await writeFile(stderrFile, "");
  const active = await exists(join(out, "active.json")) ? JSON.parse(await readFile(join(out, "active.json"), "utf8")) : null;
  const cwd = active?.name === name ? join(out, "A") : join(out, name);
  const p = Bun.spawn(args, { cwd, env, stdout: Bun.file(stdoutFile), stderr: Bun.file(stderrFile) });
  const code = await p.exited;
  const result = { code, stdout: await readFile(stdoutFile, "utf8"), stderr: await readFile(stderrFile, "utf8") };
  await writeFile(join(out, "logs", `${name}-${label}.json`), JSON.stringify({ name, label, args, seconds: (performance.now() - start) / 1000, ...result }, null, 2));
  console.log(`${name}/${label}: exit ${result.code} (${((performance.now() - start) / 1000).toFixed(1)}s)`);
  if (result.code) console.log((result.stdout + result.stderr).slice(-9000));
  await mkdir(join(out, "evidence", name), { recursive: true });
  for (const file of await readdir(join(cwd, ".hitslop")).catch(() => [])) {
    if (file.startsWith("boundary-") && file.endsWith(".json")) await copyFile(join(cwd, ".hitslop", file), join(out, "evidence", name, file));
  }
  for (const [prefix, file] of [["render", "render/results.json"], ["crash", "crash-native.json"]]) {
    const source = join(cwd, ".hitslop/evidence", file!);
    if (label.startsWith(prefix!) && await exists(source)) await copyFile(source, join(out, "evidence", name, `${prefix}.json`));
  }
  return result;
}
export async function activate(name: string) {
  const baseline = JSON.parse(await readFile(join(out, "baseline.json"), "utf8"));
  const extra = ["apps/apple/Packages/HitSlopApple/Tests/HitSlopDocumentTests/BoundarySpike.swift", "apps/apple/Packages/HitSlopApple/Tests/HitSlopCoreTests/PackageRegression.swift", "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/SlopDuplicator.swift"];
  const files = [...Object.keys(baseline.files), ...extra];
  if (!(await exists(join(out, "A-frozen")))) {
    for (const file of files) if (await exists(join(out, "A", file))) await copy(join(out, "A", file), join(out, "A-frozen", file));
  }
  const nativeGenerated = "apps/apple/Packages/HitSlopApple/Generated";
  if (!(await exists(join(out, "A-frozen", nativeGenerated)))) {
    const saved = await command(["/bin/cp", "-cR", join(out, "A", nativeGenerated), join(out, "A-frozen", nativeGenerated)]);
    if (saved.code) throw new Error(saved.stderr);
  }
  const source = name === "A" ? join(out, "A-frozen") : join(out, name);
  for (const file of files) {
    const from = join(source, file), to = join(out, "A", file);
    if (!(await exists(from))) { if (await exists(to)) await unlink(to); continue; }
    if ((await lstat(from)).isSymbolicLink()) continue;
    const bytes = await readFile(from);
    if (await exists(to) && bytes.equals(await readFile(to))) continue;
    await mkdir(dirname(to), { recursive: true });
    await writeFile(to, bytes);
  }
  const bindings = await command(["/bin/cp", "-cR", join(source, nativeGenerated) + "/.", join(out, "A", nativeGenerated)]);
  if (bindings.code) throw new Error(bindings.stderr);
  // Xcode's explicit PCM cache does not always invalidate a copied FFI header.
  const pcm = join(out, "cache/swift/out/Intermediates.noindex/SwiftExplicitPrecompiledModules");
  for (const file of await readdir(pcm).catch(() => [])) {
    if (file.startsWith("HitSlopCoreFFI-")) await unlink(join(pcm, file));
  }
  for (const file of await readdir(join(out, "A/.hitslop"))) if (file.startsWith("boundary-")) await unlink(join(out, "A/.hitslop", file));
  await writeFile(join(out, "active.json"), JSON.stringify({ name }));
}
export async function patch(name: string) {
  const base = JSON.parse(await readFile(join(out, "baseline.json"), "utf8"));
  let diff = "";
  const reference = ["B", "C", "D"].includes(name) ? "A-frozen" : "baseline";
  const files = [...Object.keys(base.files), "apps/apple/Packages/HitSlopApple/Sources/HitSlopDocument/SlopDuplicator.swift", ...(name === "validation" || name === "combined" ? ["apps/apple/Packages/HitSlopApple/Tests/HitSlopCoreTests/PackageRegression.swift"] : [])];
  for (const file of files) {
    if (!/^(apps\/apple|crates|Cargo.lock|scripts\/|packages\/)/.test(file)) continue;
    const dest = join(out, name, file);
    const before = join(out, reference, file);
    if (!(await exists(before)) && !(await exists(dest))) continue;
    const inputs: string[] = [];
    for (const [side, path] of [["before", before], ["after", dest]] as const) {
      if (!(await exists(path))) { inputs.push("/dev/null"); continue; }
      let text = await readFile(path, "utf8");
      if (["B", "C", "D"].includes(name)) {
        text = text.replace(/  \/\/ Experiment-only access:[\s\S]*?(?=  public func state)/, "");
        text = text.replace(/    \/\/\/ Spike-only adapter,[\s\S]*?(?=    pub fn sequence)/, "");
        text = text.replace(/  static let maximumBytes: Int64 = ProcessInfo[^\n]*/, "  static let maximumBytes: Int64 = 32 * 1024 * 1024");
        text = text.split("\n").filter(line => !/spikeWrittenBytes|spikeWriteMS|let started = DispatchTime.now|let writtenBytes: Int|writtenBytes = bytes.count/.test(line)).join("\n");
      }
      const input = join(out, "patch-input", side);
      await mkdir(dirname(input), { recursive: true });
      await writeFile(input, text);
      inputs.push(input);
    }
    const result = await command(["diff", "-u", "--label", await exists(before) ? `a/${file}` : "/dev/null", "--label", await exists(dest) ? `b/${file}` : "/dev/null", ...inputs]);
    if (result.code > 1) throw new Error(result.stderr);
    diff += result.stdout;
  }
  await mkdir(join(root, "archive/spikes/boundary-simplification/patches"), { recursive: true });
  await writeFile(join(root, "archive/spikes/boundary-simplification/patches", `${name}.patch`), diff);
}
if (import.meta.main) {
  const [action, name, label, ...args] = process.argv.slice(2);
  if (action === "prepare") await prepare();
  else if (action === "workspace") console.log(await workspace(name!));
  else if (action === "activate") await activate(name!);
  else if (action === "run") process.exitCode = (await run(name!, label!, args)).code;
  else if (action === "patch") await patch(name!);
  else throw new Error("prepare | workspace NAME | run NAME LABEL COMMAND... | patch NAME");
}
