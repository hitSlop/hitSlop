import { access, constants } from "node:fs/promises";
import { basename } from "node:path";

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

/** Whether `path` is an executable file. */
export async function executable(path: string): Promise<boolean> {
  try {
    await access(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
