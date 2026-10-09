import { copyFile } from "node:fs/promises";
import { join } from "node:path";
const root = import.meta.dir;
const source = join(root, "rive");
const rive = process.env.RIVE_BIN ?? "rive";
function run(...args: string[]) {
  const result = Bun.spawnSync([rive, ...args], { env: { ...process.env, RIVE_ANALYTICS: "off" }, stdout: "inherit", stderr: "inherit" });
  if (result.exitCode) throw new Error(`Rive failed (${result.exitCode})`);
}
run(source, "--verify");
run("inspect", source, "--summary");
run(source, "--once");
await copyFile(join(source, "build/little-lamp.riv"), join(root, "assets/little-lamp.riv"));
