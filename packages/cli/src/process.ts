import { access, constants } from "node:fs/promises";
import { basename, resolve } from "node:path";

export type RunOptions = {
  cwd?: string;
  env?: Record<string, string | undefined>;
  /** Pass the command's stdout through, as a worker's author logs; otherwise it is returned. */
  inherit?: boolean;
  /** The error when a failing command prints nothing on stderr. */
  failure?: string;
};

/** Starts `command`: `done` resolves to its stdout, or rejects with its stderr (or
 * `failure`); `kill` stops it. */
export function start(command: string[], options: RunOptions = {}) {
  const child = Bun.spawn(command, {
    cwd: options.cwd,
    env: options.env,
    stdin: "ignore",
    stdout: options.inherit ? "inherit" : "pipe",
    stderr: "pipe",
  });
  const done = (async () => {
    const [stdout, stderr, code] = await Promise.all([
      options.inherit ? "" : new Response(child.stdout as ReadableStream).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    if (code) throw new Error(stderr.trim() || options.failure || `${basename(command[0]!)} failed`);
    return stdout;
  })();
  return { done, kill: () => child.kill() };
}

/** Runs `command` to completion: its stdout, or its stderr (or `failure`) as the error. */
export function run(command: string[], options: RunOptions = {}): Promise<string> {
  return start(command, options).done;
}

/** The executable the environment variable `variable` names, or else the first of
 * `candidates` that exists; `missing` is the refusal when there is none. An override that
 * is not executable fails rather than falling back. */
export async function findExecutable(variable: string, candidates: string[], missing: string): Promise<string> {
  const override = process.env[variable];
  if (override !== undefined) {
    const path = resolve(override);
    if (!override || !(await executable(path))) throw new Error(`${variable} is not executable: ${override}`);
    return path;
  }
  for (const path of candidates) if (await executable(path)) return path;
  throw new Error(missing);
}

/** Whether `path` is an executable file. */
export async function executable(path: string): Promise<boolean> {
  try {
    await access(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
