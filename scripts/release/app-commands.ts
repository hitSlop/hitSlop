import { strict as assert } from "node:assert";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createDocument, debugEngine, engineRequest } from "../lib/native";
import { testProcess } from "../lib/test-process";

type Task = { $id: string; text: string; done: boolean; archived: boolean };

/** Exercise the actual app's owner, with no checkout evaluator override. Run on the
 * raw Xcode product before any packaging step can repair its missing dependencies. */
export async function verifyAppCommands(app: string) {
  const folder = await mkdtemp(join(tmpdir(), "hitslop-app-commands-"));
  const placement = {
    engine: await debugEngine(), cwd: folder,
    // Release hosts ignore test registry overrides; use the account registry on both
    // sides, as an installed app does. Only the core ever manages its lock files.
    env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin" },
  };
  const document = join(folder, "Commands.slop");
  let host: ReturnType<typeof testProcess> | undefined;
  try {
    await createDocument(join(app, "Contents/Resources/StarterTemplates/quick-checklist.slop"), document, placement);
    host = testProcess([join(app, "Contents/MacOS/hitSlop"), document], { ...placement, timeout: 60_000 });
    let exited = false;
    void host.output.finally(() => { exited = true; }).catch(() => {});
    const deadline = Date.now() + 20_000;
    while (!(await engineRequest({ method: "inspect", file: document }, placement)).info.live) {
      if (exited || Date.now() > deadline) throw new Error("App did not open its command socket");
      await Bun.sleep(30);
    }
    const tasks = async () => ((await engineRequest({ method: "get", documentPath: document }, placement)).state.value as { tasks: Task[] }).tasks;
    const initial = await tasks();
    const call = (command: string, args: Record<string, unknown> = {}) =>
      engineRequest({ method: "call", documentPath: document, command, args }, placement);
    await call("addTask", { text: "App bundle command regression" });
    const added = (await tasks()).find(task => task.text === "App bundle command regression");
    assert.ok(added, "The app did not add the task");
    await engineRequest({ method: "batch", documentPath: document,
      batch: { intents: [{ type: "set", path: ["tasks", { id: added.$id }, "done"], value: true }] } }, placement);
    await call("archiveFinished");
    assert.equal((await tasks()).find(task => task.$id === added.$id)?.archived, true);
    await call("restoreTask", { task: added.$id });
    assert.deepEqual((await tasks()).find(task => task.$id === added.$id), { ...added, done: false, archived: false });
    await call("removeTask", { task: added.$id });
    // Each CLI command is acknowledged only after saving. Reopen without the host to
    // prove these were real owner edits, not just successful evaluator responses.
    const accepted = await tasks();
    assert.equal(accepted.length, initial.length);
    assert.ok(!accepted.some(task => task.$id === added.$id));
    assert.ok(!exited && (await engineRequest({ method: "inspect", file: document }, placement)).info.live,
      "Commands must be handled by the running app, not a fallback closed owner");
    await host.stop();
    host = undefined;
    assert.deepEqual(await tasks(), accepted);
    console.log("PASS app add/file/restore/remove commands and reopen, without an evaluator override");
  } finally {
    if (host) await host.stop();
    await rm(folder, { recursive: true, force: true });
  }
}

if (import.meta.main) await verifyAppCommands(resolve(process.argv[2] ?? "generated/app/hitSlop.app"));
