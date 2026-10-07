/** Processes owned by verification and integration tests. Each has its own process group,
 * so cancellation also stops descendants that inherited its output pipes. */
import { spawn as nodeSpawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { finished } from "node:stream/promises";

export const verificationAbort = new AbortController();
const active = new Set<() => void>();
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => {
  process.exitCode = signal === "SIGINT" ? 130 : 143;
  verificationAbort.abort();
});
process.once("exit", () => { for (const kill of active) kill(); });

type Options = {
  cwd?: string;
  env?: Record<string, string | undefined>;
  stdin?: string | Uint8Array;
  timeout?: number;
  grace?: number;
  signal?: AbortSignal;
  log?: string;
  echo?: boolean;
  onOutput?: (text: string) => void;
};
export function testProcess(command: string[], options: Options = {}) {
  const child = nodeSpawn(command[0]!, command.slice(1), {
    cwd: options.cwd, env: options.env ?? process.env, detached: process.platform !== "win32",
    stdio: ["pipe", "pipe", "pipe"],
  });
  const log = options.log ? createWriteStream(options.log) : undefined;
  const logFinished = log ? finished(log) : Promise.resolve();
  void logFinished.catch(() => {});
  let stdout = "", stderr = "", timedOut = false, aborted = false, ended = false;
  let escalation: ReturnType<typeof setTimeout> | undefined;
  const kill = (signal: NodeJS.Signals) => {
    if (!child.pid) return;
    try { process.platform === "win32" ? child.kill(signal) : process.kill(-child.pid, signal); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error; }
  };
  const terminate = () => {
    if (ended || escalation) return;
    kill("SIGTERM");
    escalation = setTimeout(() => kill("SIGKILL"), options.grace ?? 1_000);
  };
  const exitCleanup = () => kill("SIGKILL");
  active.add(exitCleanup);
  const abort = () => { aborted = true; terminate(); };
  const timer = setTimeout(() => { timedOut = true; terminate(); }, options.timeout ?? 20_000);
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  const collect = (stream: "stdout" | "stderr", text: string) => {
    if (stream === "stdout") stdout += text; else stderr += text;
    log?.write(text);
    if (options.echo) process[stream].write(text);
    options.onOutput?.(text);
  };
  child.stdout.on("data", bytes => collect("stdout", bytes));
  child.stderr.on("data", bytes => collect("stderr", bytes));
  // A child may exit without accepting all of its input.
  child.stdin.on("error", () => {});
  child.stdin.end(options.stdin);
  const output = new Promise<{ stdout: string; stderr: string; code: number; timedOut: boolean; aborted: boolean }>((resolve, reject) => {
    let failure: Error | undefined;
    child.on("error", error => { failure = error; });
    child.on("close", async code => {
      ended = true;
      clearTimeout(timer); clearTimeout(escalation);
      active.delete(exitCleanup);
      verificationAbort.signal.removeEventListener("abort", abort);
      options.signal?.removeEventListener("abort", abort);
      try {
        if (log) { log.end(); await logFinished; }
        if (failure) reject(failure);
        else resolve({ stdout, stderr, code: code ?? 1, timedOut, aborted });
      } catch (error) { reject(error); }
    });
  });
  // Attach a handler immediately; callers may wait for readiness before awaiting exit.
  void output.catch(() => {});
  verificationAbort.signal.addEventListener("abort", abort, { once: true });
  options.signal?.addEventListener("abort", abort, { once: true });
  if (options.signal?.aborted || verificationAbort.signal.aborted) abort();
  return { pid: child.pid, output, stop: async () => { terminate(); return output; } };
}

export async function exec(command: string[], options: Options = {}) {
  const result = await testProcess(command, options).output;
  if (result.timedOut || result.aborted)
    throw new Error(`${command[0]} ${result.timedOut ? `timed out after ${options.timeout ?? 20_000}ms` : "was cancelled"}\n${result.stdout}${result.stderr}`);
  return result;
}
export async function run(command: string[], options: Options = {}) {
  const result = await exec(command, options);
  if (result.code) throw new Error(`${command.join(" ")} exited with ${result.code}\n${result.stdout}${result.stderr}`);
  return result.stdout;
}
