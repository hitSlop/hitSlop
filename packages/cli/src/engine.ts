import { join } from "node:path";
import { cliRoot } from "./paths";
import { findExecutable, run } from "./process";

/** The file engine (`crates/slop-engine`): it builds and reads `.slop` files by the rules
 * the app opens them with, on any platform. `HITSLOP_ENGINE` names one; otherwise the
 * build for this platform installed with the CLI, or a checkout's. */
export function findEngine(): Promise<string> {
  return findExecutable(
    "HITSLOP_ENGINE",
    [join(cliRoot, "engine", `${process.platform}-${process.arch}`, "slop-engine"), join(cliRoot, "../../target/release/slop-engine")],
    `@hitslop/cli has no file engine for ${process.platform}-${process.arch}. Reinstall @hitslop/cli.`,
  );
}

/** Runs the engine; its stdout, or its refusal as the error message. */
export async function engine(args: string[]): Promise<string> {
  return run([await findEngine(), ...args], { failure: `slop-engine ${args[0]} failed` });
}

/** Packs a build's stage into a template file at `file`, replacing only a template. */
export async function pack(stage: string, file: string): Promise<void> {
  await engine(["pack", stage, file]);
}
