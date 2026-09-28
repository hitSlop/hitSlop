import { join, resolve } from "node:path";
import { mkdir } from "node:fs/promises";
const here = import.meta.dir,
  root = resolve(here, "../../.hitslop/v1-evidence/engine-placement");
const directory = (await Bun.file(join(root, "latest.txt")).text()).trim();
const index = await Bun.file(join(directory, "index.json")).json();
const records = await Promise.all(
  index.map((entry: any) => Bun.file(join(directory, entry.file)).json()),
);
if (records.length !== 135) throw new Error(`Incomplete matrix: ${records.length}/135`);
for (const record of records) {
  if (record.sustainedTyping.savesDuringTyping < 8)
    throw new Error("A run did not maintain periodic autosave during typing");
  if (record.candidate === "native" && record.wasmRequests.length)
    throw new Error("Native candidate loaded WASM resources");
  if (record.processesMeasured < 2)
    throw new Error("WebContent process was missing from the memory sample");
}
const median = (a: number[]) => a.toSorted((a, b) => a - b)[Math.floor(a.length / 2)]!;
const stats = (a: number[]) => ({ median: median(a), min: Math.min(...a), max: Math.max(...a) });
const groups = [];
for (const rows of [1000, 5000, 40000])
  for (const windows of [1, 5, 10]) {
    const candidates = Object.fromEntries(
      ["baseline", "matched", "native"].map((candidate) => {
        const r = records.filter(
          (v: any) => v.candidate === candidate && v.rows === rows && v.windows === windows,
        );
        if (r.length !== 5) throw new Error(`Missing repetitions: ${candidate}/${rows}/${windows}`);
        return [
          candidate,
          {
            openMS: stats(r.map((v: any) => v.openMS)),
            memoryMiB: stats(r.map((v: any) => v.hostAndContentMiB)),
            processesMeasured: stats(r.map((v: any) => v.processesMeasured)),
            typingDurationMS: stats(r.map((v: any) => v.sustainedTyping.durationMS)),
            typingSaves: stats(r.map((v: any) => v.sustainedTyping.savesDuringTyping)),
            typingRenderP95MS: stats(r.map((v: any) => v.sustainedTyping.renderOpportunity.p95)),
            flushMS: stats(r.map((v: any) => v.flushMS)),
            maxPending: Math.max(...r.map((v: any) => v.sustainedTyping.maxPending)),
            drainMS: stats(r.map((v: any) => v.sustainedTyping.drainMS)),
            scenarios: Object.fromEntries(
              ["checkbox", "typing", "move", "paste"].map((scenario) => [
                scenario,
                {
                  acceptedP95MS: stats(r.map((v: any) => v.results[scenario].accepted.p95)),
                  renderOpportunityP95MS: stats(
                    r.map((v: any) => v.results[scenario].renderOpportunity.p95),
                  ),
                  engineP95MS: stats(r.map((v: any) => v.results[scenario].engine.p95)),
                },
              ]),
            ),
          },
        ];
      }),
    );
    const [native, matched] = [candidates.native, candidates.matched];
    const structuralWithin50ms = ["checkbox", "move"].every(
      (k) => native.scenarios[k].acceptedP95MS.median <= 50,
    );
    const renderWithinFrame = Object.keys(native.scenarios).every(
      (k) =>
        native.scenarios[k].renderOpportunityP95MS.median -
          matched.scenarios[k].renderOpportunityP95MS.median <=
        16.7,
    );
    groups.push({
      rows,
      windows,
      candidates,
      gates: {
        ordinary: rows <= 5000,
        structuralWithin50ms,
        structuralWithin50msEveryRun: ["checkbox", "move"].every(
          (k) => native.scenarios[k].acceptedP95MS.max <= 50,
        ),
        renderWithinFrame,
        queueDrained: native.drainMS.max < 50,
      },
    });
  }
const echo = await Promise.all(
  [0, 1, 2, 3, 4].map((i) => Bun.file(join(root, "transport", `${i}.json`)).json()),
);
const transport = [0, 50, 150].flatMap((rtt) =>
  [256, 4096, 65536].map((size) => {
    const select = (route: string) =>
      echo.map((v) =>
        v.transport.find((r: any) => r.rtt === rtt && r.size === size && r.route === route),
      );
    return {
      rtt,
      size,
      directP95MS: stats(select("direct").map((r) => r.p95)),
      hostP95MS: stats(select("host").map((r) => r.p95)),
    };
  }),
);
const audit = await Bun.file(join(root, "audit/report.json")).json();
const counters = await Bun.file(join(root, "counter-probe.json")).json();
const replayDirectory = (await Bun.file(join(root, "replay-latest.txt")).text()).trim();
const replay = await Bun.file(join(replayDirectory, "report.json")).json();
const output = join(here, "evidence");
await mkdir(output, { recursive: true });
await Bun.write(
  join(output, "results.json"),
  JSON.stringify(
    {
      environment: await Bun.file(join(directory, "environment.json")).json(),
      rawDirectory: directory,
      groups,
      transport,
      semanticAudit: audit,
      counterProbe: counters,
      durableLogReplay: replay,
      productionReady: false,
    },
    null,
    2,
  ),
);
const n = (x: number) => x.toFixed(1);
const rows = groups
  .filter((g) => g.windows === 1)
  .map(
    (g) =>
      `| ${g.rows.toLocaleString("en-US")} | ${n(g.candidates.matched.scenarios.checkbox.acceptedP95MS.median)} / ${n(g.candidates.native.scenarios.checkbox.acceptedP95MS.median)} | ${n(g.candidates.matched.scenarios.move.acceptedP95MS.median)} / ${n(g.candidates.native.scenarios.move.acceptedP95MS.median)} | ${n(g.candidates.matched.memoryMiB.median)} / ${n(g.candidates.native.memoryMiB.median)} | ${n(g.candidates.matched.openMS.median)} / ${n(g.candidates.native.openMS.median)} |`,
  )
  .join("\n");
await Bun.write(
  join(output, "table.md"),
  `Values are **matched WASM / native**. Latency columns are the median of five fresh-process p95s; memory/opening are medians. Full ranges and all window counts are in results.json.\n\n| Rows, one window | Checkbox acceptance ms | Move acceptance ms | Host + content MiB | Open ms |\n|---|---|---|---|---|\n${rows}\n`,
);
console.log(
  JSON.stringify(
    groups.map((g) => ({ rows: g.rows, windows: g.windows, gates: g.gates })),
    null,
    2,
  ),
);
