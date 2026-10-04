import { join, resolve } from "node:path";
import { cliRoot } from "./paths";
import { executable, run } from "./process";

/** The file engine (`crates/slop-engine`): it builds and reads `.slop` files by the rules
 * the app opens them with, on any platform. `HITSLOP_ENGINE` names one; otherwise the
 * build for this platform installed with the CLI at `root`, or a checkout's. */
export async function findEngine(root = cliRoot): Promise<string> {
  const override = process.env.HITSLOP_ENGINE;
  if (override !== undefined) {
    const path = resolve(override);
    if (!override || !(await executable(path))) throw new Error(`HITSLOP_ENGINE is not executable: ${override}`);
    return path;
  }
  const candidates = [
    join(root, "engine", `${process.platform}-${process.arch}`, "slop-engine"),
    join(root, "../../target/release/slop-engine"),
  ];
  for (const path of candidates) if (await executable(path)) return path;
  throw new Error(`@hitslop/cli has no file engine for ${process.platform}-${process.arch}. Reinstall @hitslop/cli.`);
}

/** Runs the engine; its stdout, or its refusal as the error message. */
export async function engine(args: string[]): Promise<string> {
  return run([await findEngine(), ...args], { failure: `slop-engine ${args[0]} failed` });
}

/** Packs a build's stage into a template file at `file`, replacing only a template. */
export async function pack(stage: string, file: string): Promise<void> {
  await engine(["pack", stage, file]);
}
