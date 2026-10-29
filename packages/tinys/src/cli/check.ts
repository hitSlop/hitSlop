import { delimiter, dirname, join, resolve } from "node:path";
import { cliRoot } from "./paths";
import { exec } from "./process";

/** The package, and its binary, that type-checks a project's Svelte and TypeScript. The
 * project installs it with TypeScript (the starter pins both in its devDependencies), and
 * `tiny check` runs that copy; the CLI carries none to fall back on. */
export const checker = "svelte-check";

/** `tiny check`: the project's checker, then tiny.ts evaluated as a build evaluates it. */
export async function check(source: string) {
  const project = resolve(source);
  const binary = projectBinary(project);
  if (!binary) throw new Error(await missingChecker(project));
  // Checkers install JavaScript launchers, which this Bun runs without Node.
  const { code } = await exec([process.execPath, binary, "--workspace", project], {
    inherit: ["stdout", "stderr"],
  });
  await (await import("./build")).checkProject(project);
  if (code) throw new Error("Authoring checks failed");
}

/** The checker `bun run` finds for the project: in the nearest node_modules/.bin. */
function projectBinary(project: string) {
  const directories: string[] = [];
  for (let directory = project; ; directory = dirname(directory)) {
    directories.push(join(directory, "node_modules", ".bin"));
    if (dirname(directory) === directory) break;
  }
  return Bun.which(checker, { PATH: directories.join(delimiter) }) ?? undefined;
}

type Manifest = { dependencies?: Record<string, string>; devDependencies?: Record<string, string> };

/** What to run when the project has no checker: its install, or the starter's pins. */
async function missingChecker(project: string) {
  const manifest: Manifest = await Bun.file(join(project, "package.json"))
    .json()
    .catch(() => ({}));
  if (manifest.devDependencies?.[checker] ?? manifest.dependencies?.[checker])
    return `tiny check runs the project's ${checker}, which is not installed. Run bun install in ${project}.`;
  const starter = await Bun.file(join(cliRoot, "templates/checklist/package.json")).json();
  const tools = Object.entries<string>(starter.devDependencies)
    .filter(([name]) => name !== "@hitslop/tinys")
    .map(([name, version]) => `${name}@${version}`);
  return `tiny check runs the project's own ${checker}, and ${project} has none. Add it and TypeScript: bun add -d ${tools.join(" ")}`;
}
