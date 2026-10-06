import { dirname, join } from "node:path";
import { homedir } from "node:os";
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

/** Document commands (create, open, get, edits, export) use the installed app's owner build
 * on macOS, or the CLI's engine on other platforms: a newer CLI never migrates a document
 * past what the installed app opens. What the author builds (validation, packing, inspect,
 * schema) always uses the CLI's own engine (`findEngine`). An explicit helper selects its
 * sibling engine as one deployment. */
export async function findDocumentEngine(): Promise<string> {
  if (process.env.HITSLOP_ENGINE !== undefined || process.platform !== "darwin") return findEngine();
  if (process.env.HITSLOP_NATIVE_CLI !== undefined) {
    const helper = await findExecutable("HITSLOP_NATIVE_CLI", [], "Missing native helper");
    return findExecutable("HITSLOP_ENGINE", [join(dirname(helper), "slop-engine")],
      `Missing slop-engine alongside ${helper}; rebuild or reinstall the native tools`);
  }
  return findExecutable("HITSLOP_ENGINE", [
    "/Applications/hitSlop.app/Contents/Helpers/slop-engine",
    join(homedir(), "Applications/hitSlop.app/Contents/Helpers/slop-engine"),
    join(cliRoot, "engine", `${process.platform}-${process.arch}`, "slop-engine"),
    join(cliRoot, "../../target/release/slop-engine"),
  ], `No document engine for ${process.platform}-${process.arch}; install hitSlop.app or reinstall @hitslop/cli`);
}
