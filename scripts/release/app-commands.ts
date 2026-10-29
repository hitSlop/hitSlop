import { buildTemplate } from "../../packages/tinys/src/cli/template";
import { strict as assert } from "node:assert";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createDocument, debugEngine, engineRequest } from "../lib/native";
import { testProcess } from "../lib/test-process";
import { request } from "../../packages/tinys/src/cli/engine";
import type { EngineRequestFor } from "../../packages/tinys/src/wire/engine";
import { copyCorpusDocument, documents, releases, readJSON, type Expected, type Scenario, type CommandProbe } from "../compat/corpus";

type Task = { $id: string; text: string; done: boolean; archived: boolean };

/** Exercise the actual app's owner, with no checkout evaluator override. Run on the
 * raw Xcode product before any packaging step can repair its missing dependencies. */
export async function verifyAppCommands(app: string) {
  const folder = await mkdtemp(join(tmpdir(), "tinys-app-commands-"));
  const placement = {
    engine: await debugEngine(), cwd: folder,
    // Release hosts ignore test registry overrides; use the account registry on both
    // sides, as an installed app does. Only the core ever manages its lock files.
    env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin" },
  };
  const document = join(folder, "Commands.tiny");
  let host: ReturnType<typeof testProcess> | undefined;
  try {
    const template = await buildTemplate(resolve("tests/apps/document"), join(folder, "fixture.tiny"), { renderViews: false });
    await createDocument(template, document, placement);
    host = testProcess([join(app, "Contents/MacOS/tinys"), document], { ...placement, timeout: 60_000 });
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
    await call("addRow", { text: "App bundle command regression" });
    const added = (await tasks()).find(task => task.text === "App bundle command regression");
    assert.ok(added, "The app did not add the task");
    await engineRequest({ method: "batch", documentPath: document,
      batch: { intents: [{ type: "set", path: ["tasks", { id: added.$id }, "done"], value: true }] } }, placement);
    await call("resetRow", { task: added.$id });
    assert.deepEqual((await tasks()).find(task => task.$id === added.$id), { ...added, done: false, archived: false });
    await call("removeRow", { task: added.$id });
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
    console.log("PASS app insert/row-ID/remove commands and reopen, without an evaluator override");
  } finally {
    if (host) await host.stop();
    await rm(folder, { recursive: true, force: true });
  }
}

/** Packaged owner/evaluator acceptance. The old page's contractTest remains the
 * responsibility of CompatCorpusTests; this checks the actual signed application's
 * socket routing, original command responses, refusal semantics and persisted edits. */
export async function verifyAppCorpus(app: string, options: { allowUnreleased?: boolean } = {}) {
  const entries = await releases();
  if (!entries.length && options.allowUnreleased && process.env.TINYS_COMPAT_BOOTSTRAP === "1" && !process.env.TINYS_RELEASE_TAG) {
    console.log("No published tinys corpus yet; unsigned development app acceptance uses its live-owner fixture.");
    return;
  }
  assert.ok(entries.length, "No corpus for packaged-app acceptance");
  const folder = await mkdtemp(join(tmpdir(), "tinys-app-corpus-"));
  const placement = { engine: await debugEngine(), cwd: folder,
    env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin" } };
  let host: ReturnType<typeof testProcess> | undefined;
  try {
    for (const entry of entries) for (const name of await documents(entry.root)) {
      const source = join(entry.root, "documents", `${name}.tiny`);
      const expected = await readJSON<Expected>(join(entry.root, "expected", `${name}.json`));
      const scenario = await readJSON<Scenario>(join(entry.root, "scenarios", `${name}.json`));
      assert.ok(expected && scenario, `${entry.name}/${name}: missing expected state or edit scenario`);
      const probes = (await readJSON<CommandProbe[]>(join(entry.root, "commands", `${name}.json`)) ?? []).filter(probe => probe.channel === "call");
      for (const probe of [undefined, ...probes]) {
        const document = join(folder, "Historical.tiny");
        await rm(document, { force: true });
        await copyCorpusDocument(source, document);
        host = testProcess([join(app, "Contents/MacOS/tinys"), document], { ...placement, timeout: 90_000 });
        let exited = false;
        void host.output.finally(() => { exited = true; }).catch(() => {});
        const live = async () => !exited && (await engineRequest({ method: "inspect", file: document }, placement)).info.live;
        const deadline = Date.now() + 30_000;
        while (!(await live())) {
          if (exited || Date.now() > deadline) throw new Error(`App did not open ${entry.name}/${name}`);
          await Bun.sleep(30);
        }
        const value = async () => (await engineRequest({ method: "get", documentPath: document }, placement)).state.value;
        assert.deepEqual(await value(), expected.value);
        if (probe) {
          const outcome = await request({ method: "call", documentPath: document, command: probe.name, args: probe.args }, { binary: placement.engine, cwd: folder, env: placement.env });
          assert.equal(outcome.ok, probe.expected.ok, `${entry.name}/${name}: ${probe.name}`);
          if (outcome.ok) assert.deepEqual(outcome.result, (probe.evaluated as { result: unknown }).result);
          else {
            if (probe.expected.code) assert.equal(outcome.code, probe.expected.code);
            if (probe.expected.refused) assert.equal(outcome.reason, "refused");
            if (probe.expected.errorIncludes) assert.ok(outcome.error.includes(probe.expected.errorIncludes));
          }
          assert.deepEqual(await value(), probe.value);
        } else {
          await engineRequest({ method: "batch", documentPath: document,
            batch: { intents: scenario.ops } as EngineRequestFor<"batch">["batch"] }, placement);
          assert.deepEqual(await value(), scenario.value);
        }
        assert.ok(await live(), "Historical commands must use the running app, not a fallback closed owner");
        const saved = await value();
        await host.stop();
        host = undefined;
        assert.deepEqual(await value(), saved);
        console.log(`PASS packaged corpus ${entry.name}/${name}: ${probe ? `call ${probe.name}` : "edit/save/reopen"}`);
      }
    }
  } finally {
    if (host) await host.stop();
    await rm(folder, { recursive: true, force: true });
  }
}

if (import.meta.main) await verifyAppCommands(resolve(process.argv[2] ?? "generated/app/tinys.app"));
