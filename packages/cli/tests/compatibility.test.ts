// Guards false-green replay: wrong expected data, broken authored handles, and rewritten release history.
import { test, expect } from "bun:test";
import { cp, mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { buildRuntime } from "../../../scripts/v1/runtime";
import { runRuntime } from "../../../scripts/v1/compatibility";
import { checkHistory } from "../../../scripts/v1/compatibility-history";
import { loadRuntime } from "../../../scripts/v1/compatibility-worker";
import { SQLiteStore } from "../../document/test-support/sqlite";
import { defineDocument, s } from "../../document/src/schema";
import { digest, releases, repository } from "../../../scripts/v1/runtime-artifacts";
import identity from "../../document/src/runtime-identity.json";

test("historical readers open JSON-imported updates and checkpoints with preserved rich text and references", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-import-readers-"));
  try {
    const runtimes = join(root, "runtimes");
    await buildRuntime([runtimes]);
    const contract = String(identity.runtimeContract);
    const runtime = await loadRuntime(join(runtimes, contract));
    const schema = defineDocument({
      title: s.text(),
      notes: s.richtext({ bold: "after" }),
      count: s.counter(),
      rows: s.list(s.object({ label: s.string(), link: s.string() })),
      tree: s.tree(s.object({ label: s.string() })),
    });
    const initial = { title: "Old", notes: "", count: 0, rows: [], tree: [] };
    for (const phase of ["updates", "checkpoint"]) {
      const document = join(root, phase + ".slop");
      await mkdir(document);
      await writeFile(join(document, "state.schema.json"), JSON.stringify(schema.descriptor));
      await writeFile(join(document, "initial.json"), JSON.stringify(initial));
      const store = await SQLiteStore.open(document);
      const doc = await runtime.Document.open(
        runtime.fromDescriptor(schema.descriptor),
        store,
        initial,
      );
      const baseline = await store.load(),
        version = doc.version();
      doc.importJSON({
        title: "Imported",
        notes: { text: "Hello", delta: [{ insert: "Hello", attributes: { bold: true } }] },
        count: 4,
        rows: [{ label: "Linked", link: { $ref: "/tree/0/children/0" } }],
        tree: [{ label: "Root", children: [{ label: "Child" }] }],
      });
      expect(doc.current.title).toBe("Imported");
      expect(doc.current.notes.delta).toEqual([{ insert: "Hello", attributes: { bold: true } }]);
      expect(doc.current.count).toBe(4);
      expect(doc.current.rows[0].link).toBe(doc.current.tree[0].children[0].$id);
      const expected = join(root, phase + ".json");
      await writeFile(expected, JSON.stringify(doc.current));
      const importedUpdates = doc.exportUpdates(version);
      await doc.close();
      // Imports checkpoint by default. Independently construct an update-only reader
      // specimen to also prove the generated CRDT operations remain readable contract data.
      let readerSource = document;
      if (phase === "updates") {
        readerSource = join(root, "updates-only.slop");
        await mkdir(readerSource);
        await writeFile(join(readerSource, "state.schema.json"), JSON.stringify(schema.descriptor));
        await writeFile(join(readerSource, "initial.json"), JSON.stringify(initial));
        const readerStore = await SQLiteStore.open(readerSource);
        const generation = await readerStore.checkpoint(
          "0",
          baseline.checkpoint!,
          baseline.schemaKey!,
        );
        await readerStore.append(generation, [importedUpdates]);
        await readerStore.close();
      }
      for (const release of (await releases()).filter((r) => r.runtimeContract === identity.runtimeContract)) {
        // Fresh runners restore older readers only. The newly sealed current
        // revision comes from this candidate build and must match its ledger seal.
        const historical = release.runtimeContract === identity.runtimeContract &&
          release.runtimeRevision === identity.runtimeRevision
          ? join(runtimes, contract)
          : join(repository, "generated/v1/runtime-releases", `${contract}-${release.runtimeRevision}`, contract);
        expect(await digest(historical)).toBe(release.sha256);
        const copy = join(root, `${phase}-reader-${release.runtimeRevision}.slop`);
        await cp(readerSource, copy, { recursive: true });
        const child = Bun.spawn(
          [process.execPath, "scripts/v1/compatibility-worker.ts", historical],
          {
            stdin: new Blob([JSON.stringify([{ document: copy, expected }])]),
            stdout: "pipe",
            stderr: "pipe",
          },
        );
        const [out, error, code] = await Promise.all([
          new Response(child.stdout).text(),
          new Response(child.stderr).text(),
          child.exited,
        ]);
        expect(code, error).toBe(0);
        expect(JSON.parse(out)[0].state).toEqual(JSON.parse(await Bun.file(expected).text()));
      }
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60000);

test("compiled replay detects incorrect expectations and a no-op authored text handle", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-replay-fault-"));
  try {
    const runtimes = join(root, "runtimes");
    await buildRuntime([runtimes]);
    const current = join(runtimes, String(identity.runtimeContract));
    const document = join(root, "Document.slop");
    await cp("tests/compatibility/2-1/document", document, { recursive: true });
    const expected = await Bun.file("tests/compatibility/2-1/expected.json").json();
    const wrong = join(root, "wrong.json");
    await writeFile(wrong, JSON.stringify({ ...expected, title: "Wrong saved state" }));
    await expect(runRuntime(current, document, wrong)).rejects.toThrow(
      `Document ${document}, read: AssertionError`,
    );
    // The same process must release ownership after a failed assertion.
    const good = await runRuntime(
      current,
      document,
      "tests/compatibility/2-1/expected.json",
    );
    expect(good.state).toEqual(expected);
    // Mutate only a disposable compiled runtime; seals must never be regenerated.
    const runtime = join(root, "mutant");
    await cp(current, runtime, { recursive: true });
    await cp(join(runtime, "index.js"), join(runtime, "implementation.js"));
    await writeFile(
      join(runtime, "index.js"),
      `
      export * from './implementation.js';
      import {Document} from './implementation.js';
      const open = Document.open;
      Document.open = async function(...args) {
        const doc = await open.apply(this, args);
        doc.fields = {...doc.fields, title: {...doc.fields.title, replace() {}}};
        return doc;
      };
    `,
    );
    await expect(
      runRuntime(
        runtime,
        document,
        "tests/compatibility/2-1/expected.json",
        "tests/compatibility/2-1/scenario.json",
      ),
    ).rejects.toThrow("AssertionError");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 60_000);

test("committed fixture and release rewrites fail against an independent base commit", async () => {
  const root = await mkdtemp(join(tmpdir(), "hitslop-history-fault-"));
  async function git(...args: string[]) {
    const child = Bun.spawn(["git", ...args], { cwd: root, stdout: "pipe", stderr: "pipe" });
    const [out, error, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    expect(code, error).toBe(0);
    return out.trim();
  }
  try {
    await git("init");
    await mkdir(join(root, "runtimes"));
    await mkdir(join(root, "tests/compatibility/example/document/assets"), { recursive: true });
    const records = [{ runtimeContract: 2, runtimeRevision: 1, sha256: "a".repeat(64) }];
    await writeFile(join(root, "runtimes/releases.json"), JSON.stringify(records));
    await writeFile(join(root, "tests/compatibility/example/document/assets/app.js"), "Original app");
    for (const oracle of ["issues.json", "collaboration.json"])
      await writeFile(join(root, "tests/compatibility/example", oracle), "recorded");
    await git("add", ".");
    await git(
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "-c",
      "commit.gpgsign=false",
      "commit",
      "-m",
      "baseline",
    );
    const base = await git("rev-parse", "HEAD");
    await checkHistory(root, base);
    await writeFile(join(root, "runtimes/releases.json"), "[]");
    await git("add", ".");
    await git(
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "-c",
      "commit.gpgsign=false",
      "commit",
      "-m",
      "rewrite release",
    );
    await expect(checkHistory(root, base)).rejects.toThrow("Published runtime record changed");
    await writeFile(join(root, "runtimes/releases.json"), JSON.stringify(records));
    await writeFile(join(root, "tests/compatibility/example/document/assets/app.js"), "Rebuilt app");
    await expect(checkHistory(root, base)).rejects.toThrow("Preserved fixture changed");
    await writeFile(join(root, "tests/compatibility/example/document/assets/app.js"), "Original app");
    await checkHistory(root, base);
    // Behavioral oracles beside the document are protected too.
    for (const oracle of ["issues.json", "collaboration.json"]) {
      await writeFile(join(root, "tests/compatibility/example", oracle), "[]");
      await expect(checkHistory(root, base)).rejects.toThrow("Preserved fixture changed");
      await writeFile(join(root, "tests/compatibility/example", oracle), "recorded");
      await checkHistory(root, base);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
