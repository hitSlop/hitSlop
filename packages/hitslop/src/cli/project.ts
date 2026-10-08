import { realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { cliRoot } from "./paths";
import metadata from "../../package.json";

export function projectPackage(directory: string): string {
  try { return Bun.resolveSync("hitslop/package.json", resolve(directory)); }
  catch { throw new Error("Install this project's dependencies with bun install, then use its bun run check/dev/build scripts."); }
}

/** Authoring uses the project's SDK; ordinary document commands never consult it. */
export function assertProjectPackage(directory: string) {
  const path = projectPackage(directory);
  const installed = require(path) as { version: string };
  if (installed.version !== metadata.version)
    throw new Error(`This project pins hitslop ${installed.version}; this CLI is ${metadata.version}. Use the project's bun run check/dev/build scripts, or slop --project=${resolve(directory)} build .`);
}

/** An explicit project prefix is the only delegation rule, read before command parsing. */
export async function delegate(argv: string[]): Promise<string[]> {
  const prefix = argv[0];
  if (!prefix?.startsWith("--project=")) return argv;
  const directory = prefix.slice("--project=".length);
  if (!directory) throw new Error("--project=DIR requires a project directory");
  const target = dirname(projectPackage(directory));
  const rest = argv.slice(1);
  const cwd = resolve(directory);
  if (realpathSync(target) === realpathSync(cliRoot)) {
    process.chdir(cwd);
    return rest;
  }
  const child = Bun.spawn([process.execPath, resolve(target, "src/cli/cli.ts"), ...rest], {
    cwd, stdin: "inherit", stdout: "inherit", stderr: "inherit",
  });
  process.exit(await child.exited);
}
