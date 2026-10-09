/** Local, opt-in authority proof. Run through Bun; never included in a released app. */
import { mkdtemp, mkdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { stageWorker } from "../../packages/hitslop/src/cli/build";
import { execute, negotiate } from "../../packages/hitslop/src/cli/engine";
import { startDev } from "../../packages/hitslop/src/cli/dev";
import type { PreviewOwnerFactory } from "../../packages/hitslop/src/cli/preview-owner";
import { run } from "../../packages/hitslop/src/cli/process";

type Peer = "a" | "b";
type Action = "disconnect" | "pauseDelivery" | "resumeDelivery" | "duplicateNext" | "skipNext" | "snapshot";
const repository = resolve(import.meta.dir, "../..");
export const devSyncEngine = join(repository, "target/dev-sync/release/slop-engine");

/** Builds the engine and, beside it, the `slop-room` development binary. */
export async function buildDevSyncEngine() {
  await run(["cargo", "build", "--locked", "--release", "-p", "slop-engine", "--features", "dev-sync", "--bins",
    "--target-dir", join(repository, "target/dev-sync")], { cwd: repository });
  return devSyncEngine;
}

/** The connection descriptor is opaque: only Rust understands the private room wire. */
export async function startLiveSync(source: string, options: { engine?: string; directory?: string } = {}) {
  const engine = options.engine ?? devSyncEngine;
  // The room and replica owners run in `slop-room`, built beside the engine.
  const roomCommand = (role: string, path: string) => [join(dirname(engine), "slop-room"), ...negotiate(engine).slice(1), role, path];
  const directory = options.directory ? resolve(options.directory) : await mkdtemp(join(tmpdir(), "hitslop-live-sync-"));
  await mkdir(directory, { recursive: true });
  const paths = { authority: join(directory, "authority.slop"), a: join(directory, "replica-a.slop"), b: join(directory, "replica-b.slop") };
  const stage = join(directory, "stage");
  const built = await stageWorker(source, stage, "Live sync fixture build failed").done;
  const template = join(directory, "template.slop");
  await execute({ method: "pack", app: built.input, stage, file: template }, { binary: engine });
  await execute({ method: "create", from: template, output: paths.authority }, { binary: engine });
  const room = Bun.spawn(roomCommand("--dev-room", paths.authority), { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
  const views: Awaited<ReturnType<typeof startDev>>[] = [];
  const pending = new Map<number, { resolve(): void; reject(error: unknown): void; timer: ReturnType<typeof setTimeout> }>();
  const ready = Promise.withResolvers<Record<string, unknown>>();
  let next = 0, stopped = false, failed: Error | undefined, closing: Promise<void> | undefined;
  const stderr = new Response(room.stderr).text();
  const fail = (error: unknown) => {
    failed = error instanceof Error ? error : new Error(String(error));
    ready.reject(failed);
    for (const request of pending.values()) { clearTimeout(request.timer); request.reject(failed); }
    pending.clear();
  };
  const read = (async () => {
    const reader = room.stdout.getReader();
    const decoder = new TextDecoder();
    let buffered = "";
    try {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buffered += decoder.decode(value, { stream: true });
        if (buffered.length > 1024 * 1024) throw new Error("Oversized room control response");
        let end: number;
        while ((end = buffered.indexOf("\n")) !== -1) {
          const frame = JSON.parse(buffered.slice(0, end)); buffered = buffered.slice(end + 1);
          if (frame.type === "ready" && frame.connection && typeof frame.connection === "object") ready.resolve(frame.connection);
          else if (frame.type === "control") {
            const request = pending.get(frame.id);
            if (request) { pending.delete(frame.id); clearTimeout(request.timer); frame.ok ? request.resolve() : request.reject(new Error(frame.error ?? "Room control failed")); }
          } else throw new Error("Unexpected room control response");
        }
      }
      if (!stopped) throw new Error(`Room stopped (${await room.exited}): ${await stderr}`);
    } catch (error) { fail(error); }
    finally { reader.releaseLock(); }
  })();
  const close = () => closing ??= (async () => {
    stopped = true;
    const results = await Promise.allSettled(views.map(view => view.close()));
    fail(new Error("Live sync harness closed"));
    room.stdin.end();
    const kill = setTimeout(() => room.kill(), 12000);
    try {
      const code = await room.exited;
      await read;
      const errors = await stderr;
      if (code) throw new Error(`Room close failed (${code}): ${errors}`);
    }
    finally { clearTimeout(kill); }
    const rejected = results.find(result => result.status === "rejected");
    if (rejected?.status === "rejected") throw rejected.reason;
  })();
  const startup = setTimeout(() => ready.reject(new Error("Room startup timed out")), 15000);
  try {
    room.stdin.write(JSON.stringify({ backups: [paths.a, paths.b] }) + "\n");
    await room.stdin.flush();
    const connection = await ready.promise;
    clearTimeout(startup);
    for (const peer of ["a", "b"] as const) {
      let leased = false;
      const factory: PreviewOwnerFactory = async () => {
        if (leased) throw new Error(`Replica ${peer} already has a browser view`);
        if (stopped || failed) throw failed ?? new Error("Room closed");
        leased = true;
        return { path: paths[peer], command: roomCommand("--dev-replica-owner", paths[peer]),
          startup: { ...connection, peer }, async dispose() { leased = false; } };
      };
      views.push(await startDev(source, 0, undefined, factory));
    }
    return {
      directory, paths, urls: { a: views[0]!.url, b: views[1]!.url },
      diagnostics: () => Promise.all(views.map(view => view.diagnostics())),
      control(action: Action, peer: Peer) {
        if (stopped || failed) return Promise.reject(failed ?? new Error("Room closed"));
        if (pending.size >= 16) return Promise.reject(new Error("Room control queue full"));
        const id = ++next;
        return new Promise<void>((resolve, reject) => {
          const timer = setTimeout(() => { pending.delete(id); reject(new Error("Room control timed out")); }, 15000);
          pending.set(id, { resolve, reject, timer });
          try {
            room.stdin.write(JSON.stringify({ id, action, peer }) + "\n");
            void Promise.resolve(room.stdin.flush()).catch(fail);
          } catch (error) { fail(error); }
        });
      }, close,
    };
  } catch (error) { await close(); throw error; }
  finally { clearTimeout(startup); }
}

if (import.meta.main) {
  const source = resolve(process.argv[2] ?? "examples/slops/quick-checklist");
  const engine = await buildDevSyncEngine();
  const harness = await startLiveSync(source, { engine });
  console.log(`Replica A: ${harness.urls.a}\nReplica B: ${harness.urls.b}\nFiles: ${harness.directory}\nDevelopment proof only; keep both owners running while editing.`);
  await new Promise<void>(resolve => {
    process.once("SIGINT", resolve); process.once("SIGTERM", resolve);
  });
  await harness.close();
}
