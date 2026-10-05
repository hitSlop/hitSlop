import { access, constants } from "node:fs/promises";
import { basename, resolve } from "node:path";

type RunOptions = {
  cwd?: string;
  env?: Record<string, string | undefined>;
  /** The command's standard input; without it, the command reads none. */
  stdin?: string | Uint8Array;
  /** Streams the command shares with this process instead of returning: its input, an
   * author's worker logs, a helper's own messages. */
  inherit?: ("stdin" | "stdout" | "stderr")[];
  /** Kills the command and fails after this many milliseconds. */
  timeout?: number;
  /** The error when a failing command prints nothing on stderr. */
  failure?: string;
};
/** What a command printed, and how it exited. */
type Output = { stdout: string; stderr: string; code: number };

/** Starts `command`: `output` resolves once it exits, whatever its status; `kill` stops it. */
export function spawn(command: string[], options: RunOptions = {}) {
  const shared = (stream: "stdin" | "stdout" | "stderr") => options.inherit?.includes(stream) ?? false;
  const child = Bun.spawn(command, {
    cwd: options.cwd,
    env: options.env,
    stdin: shared("stdin") ? "inherit" : options.stdin === undefined ? "ignore" : Buffer.from(options.stdin),
    stdout: shared("stdout") ? "inherit" : "pipe",
    stderr: shared("stderr") ? "inherit" : "pipe",
  });
  let timedOut = false;
  const timer = options.timeout === undefined ? undefined : setTimeout(() => ((timedOut = true), child.kill()), options.timeout);
  const text = (stream: unknown) => (stream instanceof ReadableStream ? new Response(stream).text() : "");
  const output = (async (): Promise<Output> => {
    try {
      const [stdout, stderr, code] = await Promise.all([text(child.stdout), text(child.stderr), child.exited]);
      if (timedOut) throw new Error(`${basename(command[0]!)} did not finish within ${options.timeout! / 1000} s`);
      return { stdout, stderr, code };
    } finally {
      clearTimeout(timer);
    }
  })();
  return { output, kill: () => child.kill() };
}

/** Runs `command` to completion: what it printed and its exit status, whatever the status. */
export function exec(command: string[], options: RunOptions = {}): Promise<Output> {
  return spawn(command, options).output;
}

/** Starts `command`: `done` resolves to its stdout, or rejects with its stderr (or
 * `failure`); `kill` stops it. */
export function start(command: string[], options: RunOptions = {}) {
  const { output, kill } = spawn(command, options);
  const done = output.then(({ stdout, stderr, code }) => {
    if (code) throw new Error(stderr.trim() || options.failure || `${basename(command[0]!)} failed`);
    return stdout;
  });
  return { done, kill };
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
async function executable(path: string): Promise<boolean> {
  try {
    await access(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}
