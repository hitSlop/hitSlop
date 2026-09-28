import { cp, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { buildRuntime } from "./runtime";
import { checkCompatibility } from "./compatibility-check";
import { catalog, digest, releases, repository } from "./runtime-artifacts";
import { checkHistory } from "./compatibility-history";
import { discoverTemplates } from "./templates";
import { buildProject } from "../../packages/cli/src/build";

import {
  canonical,
  loadRuntime,
  runCase,
  runPeer,
  type PeerCase,
  type RuntimeCase,
  type RuntimeResult,
} from "./compatibility-worker";

export type ReplayResult = {
  fixture: string;
  runtime: string;
  phase: string;
  stateHash: string;
  equivalentReleases?: string[];
};
export async function runRuntime(
  runtime: string,
  document: string,
  expected = "-",
  scenario = "-",
  phase = "read",
  issues?: string,
) {
  return runCase(await loadRuntime(runtime), { document, expected, scenario, phase, issues });
}

async function readHistorical(
  runtime: string,
  cases: (RuntimeCase | PeerCase)[],
): Promise<RuntimeResult[]> {
  const child = Bun.spawn(
    [process.execPath, join(import.meta.dir, "compatibility-worker.ts"), runtime],
    { stdin: new Blob([JSON.stringify(cases)]), stdout: "pipe", stderr: "pipe" },
  );
  const [out, error, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (code) throw new Error(`Runtime ${runtime}: ${error || out}`);
  return JSON.parse(out);
}

export async function replayCompatibility(
  runtimeRoot: string,
  corpus = join(repository, "tests/compatibility"),
) {
  const installed = await catalog(runtimeRoot);
  const published = await releases();
  const workspace = await mkdtemp(join(tmpdir(), "hitslop-replay-"));
  const results: ReplayResult[] = [];
  const readers = new Map<
    string,
    {
      runtime: string;
      cases: RuntimeCase[];
      results: Omit<ReplayResult, "stateHash">[];
    }
  >();
  try {
    // Verify seals before using hash equality to avoid redundant reader executions.
    const historical = await Promise.all(
      published.map(async (release) => {
        const key = `${release.runtimeContract}-${release.runtimeRevision}`;
        const current = installed[String(release.runtimeContract)];
        const runtime =
          current?.identity.runtimeRevision === release.runtimeRevision
            ? join(runtimeRoot, String(release.runtimeContract))
            : join(
                repository,
                "generated/v1/runtime-releases",
                key,
                String(release.runtimeContract),
              );
        const sha256 = await digest(runtime);
        if (sha256 !== release.sha256) throw new Error(`Restore immutable runtime ${key}`);
        return { ...release, key, runtime };
      }),
    );
    for (const name of (await readdir(corpus)).sort()) {
      const fixture = join(corpus, name);
      if (!(await Bun.file(join(fixture, "fixture.json")).exists())) continue;
      const record = await Bun.file(join(fixture, "fixture.json")).json();
      if ((await digest(join(fixture, "document"))) !== record.sha256)
        throw new Error(`Preserved fixture changed: ${name}`);
      const current = installed[String(record.runtimeContract)];
      if (!current || current.identity.runtimeRevision < record.runtimeRevision)
        throw new Error(`Unsupported fixture: ${name}`);
      const candidate = join(runtimeRoot, String(record.runtimeContract));
      const scenario = join(fixture, "scenario.json");
      const hasScenario = await Bun.file(scenario).exists();
      for (const phase of hasScenario ? ["updates", "checkpoint"] : ["read"]) {
        const document = join(workspace, `${name}-${phase}.slop`);
        await cp(join(fixture, "document"), document, { recursive: true });
        const issues = join(fixture, "issues.json");
        const result = await runRuntime(
          candidate,
          document,
          join(fixture, "expected.json"),
          hasScenario ? scenario : "-",
          phase,
          (await Bun.file(issues).exists()) ? issues : undefined,
        );
        const candidateResult: ReplayResult = {
          fixture: name,
          runtime: `${record.runtimeContract}-${current.identity.runtimeRevision}`,
          phase,
          stateHash: result.stateHash,
        };
        results.push(candidateResult);
        const expected = join(workspace, `${name}-${phase}.json`);
        await writeFile(expected, JSON.stringify(result.state));
        // Different readers get copies of the same candidate-written bytes.
        for (const release of historical.filter(
          (r) =>
            r.runtimeContract === record.runtimeContract &&
            r.runtimeRevision >= record.runtimeRevision,
        )) {
          if (release.sha256 === current.sha256) {
            (candidateResult.equivalentReleases ??= []).push(release.key);
            continue;
          }
          let reader = readers.get(release.sha256);
          if (!reader) {
            reader = { runtime: release.runtime, cases: [], results: [] };
            readers.set(release.sha256, reader);
          }
          const copy = join(workspace, `${name}-${phase}-${release.key}.slop`);
          await cp(document, copy, { recursive: true });
          reader.cases.push({ document: copy, expected, phase: `historical-${phase}` });
          reader.results.push({
            fixture: name,
            runtime: release.key,
            phase: `historical-${phase}`,
          });
        }
      }
    }
    for (const reader of readers.values()) {
      const reads = await readHistorical(reader.runtime, reader.cases);
      if (reads.length !== reader.results.length) throw new Error("Incomplete historical replay");
      reads.forEach((read, index) =>
        results.push({ ...reader.results[index]!, stateHash: read.stateHash }),
      );
    }
    if (!results.length) throw new Error("Compatibility corpus is empty");
    // Mixed-version collaboration: edit, exchange, edit again and reopen on both sides.
    for (const name of (await readdir(corpus)).sort()) {
      const fixture = join(corpus, name);
      if (!(await Bun.file(join(fixture, "collaboration.json")).exists())) continue;
      const record = await Bun.file(join(fixture, "fixture.json")).json();
      const current = installed[String(record.runtimeContract)]!;
      const candidate = join(runtimeRoot, String(record.runtimeContract));
      const partners = [
        { key: `${record.runtimeContract}-${current.identity.runtimeRevision}`, runtime: candidate, historical: false },
        ...historical
          .filter((r) => r.runtimeContract === record.runtimeContract && r.sha256 !== current.sha256)
          .map((r) => ({ key: r.key, runtime: r.runtime, historical: true })),
      ];
      for (const partner of partners)
        results.push(await collaborate(name, fixture, candidate, partner, workspace));
    }
    return results;
  } finally {
    await rm(workspace, { recursive: true, force: true });
  }
}

/** Peer "a" is the candidate; peer "b" is the partner runtime, isolated when historical. */
async function collaborate(
  name: string,
  fixture: string,
  candidate: string,
  partner: { key: string; runtime: string; historical: boolean },
  workspace: string,
): Promise<ReplayResult> {
  const { steps, expected } = await Bun.file(join(fixture, "collaboration.json")).json();
  const directory = await mkdtemp(join(workspace, `${name}-collaboration-`));
  const peers = {
    a: { document: join(directory, "a.slop"), runtime: candidate, historical: false, exports: join(directory, "a.bin") },
    b: { document: join(directory, "b.slop"), runtime: partner.runtime, historical: partner.historical, exports: join(directory, "b.bin") },
  };
  for (const peer of Object.values(peers))
    await cp(join(fixture, "document"), peer.document, { recursive: true });
  // `sync: false` edits offline, without first importing the other peer's history.
  const run = async (side: "a" | "b", operations: unknown[], sync = true) => {
    const peer = peers[side];
    const other = peers[side === "a" ? "b" : "a"];
    const input: PeerCase = {
      kind: "peer",
      document: peer.document,
      imports: sync && (await Bun.file(other.exports).exists()) ? [other.exports] : [],
      operations,
      exportTo: peer.exports,
    };
    return peer.historical
      ? (await readHistorical(peer.runtime, [input]))[0]!
      : runPeer(await loadRuntime(peer.runtime), input);
  };
  for (const step of steps) await run(step.peer, step.operations, step.sync ?? true);
  const a = await run("a", []);
  const b = await run("b", []);
  if (a.stateHash !== b.stateHash)
    throw new Error(`Peers diverged: ${name} between candidate and ${partner.key}`);
  // Equal peers could still have lost the same edits; compare an independent model.
  for (const [key, value] of Object.entries(expected ?? {}))
    if (canonical((a.state as Record<string, unknown>)[key]) !== canonical(value))
      throw new Error(`Collaboration result differs at ${key}: ${name} with ${partner.key}`);
  return { fixture: name, runtime: partner.key, phase: "collaboration", stateHash: a.stateHash };
}

export async function smokeTemplates(runtimeRoot: string) {
  const workspace = await mkdtemp(join(tmpdir(), "hitslop-template-smoke-"));
  const results: string[] = [];
  try {
    const pending = (await discoverTemplates()).filter((t) => t.bundled);
    const workers = await Promise.allSettled(
      Array.from({ length: 2 }, async () => {
        for (let template; (template = pending.shift());) {
          const document = await buildProject(
            template.source,
            join(workspace, `${template.slug}.slop`),
          );
          const requirements = await Bun.file(join(document, "assets/runtime.json")).json();
          await runRuntime(join(runtimeRoot, String(requirements.runtimeContract)), document);
          results.push(template.slug);
        }
      }),
    );
    for (const worker of workers) if (worker.status === "rejected") throw worker.reason;
    if (!results.length) throw new Error("No bundled templates were exercised");
    return results.sort();
  } finally {
    await rm(workspace, { recursive: true, force: true });
  }
}

export async function checkRuntime(options: { templates?: boolean; report?: string } = {}) {
  const baseline = await checkHistory();
  const root = await mkdtemp(join(tmpdir(), "hitslop-candidate-"));
  try {
    const runtimes = join(root, "runtimes");
    await buildRuntime([runtimes]);
    await checkCompatibility(runtimes, false);
    const results = await replayCompatibility(runtimes);
    const templates = options.templates === false ? [] : await smokeTemplates(runtimes);
    const report = { baseline, runtimes: await catalog(runtimes), results, templates };
    if (options.report) {
      await mkdir(resolve(options.report, ".."), { recursive: true });
      await writeFile(options.report, JSON.stringify(report, null, 2) + "\n");
    }
    console.log(
      `Compatibility: ${results.length} replay cases; ${templates.length} bundled template open/reopen checks`,
    );
    return report;
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

if (import.meta.main) await checkRuntime({ report: process.argv[2] });
