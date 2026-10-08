import { mkdir, readFile, cp, rename, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";
const here = import.meta.dir;
const rust = process.argv.includes("--rust");
const candidates = rust ? ["matched", "native", "rust-core"] : ["baseline", "matched", "native"];
const env = {...process.env, PATH:`${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${process.env.PATH}`};
const mode = process.argv.includes("--matrix") ? "matrix" : "smoke";
const resumeAt = process.argv.indexOf("--resume");
const resuming = resumeAt >= 0;
const redoAt = process.argv.indexOf("--redo");
const redo = new Set(redoAt >= 0 ? process.argv[redoAt+1]!.split(",") : []);
if (redo.size && !resuming) throw new Error("--redo requires --resume");
for (const file of redo) if (!/^[0-4]-(1000|5000|40000)-(1|5|10)-(matched|native|rust-core|baseline)\.json$/.test(file)) throw new Error("Invalid redo cell");
const destination = resuming ? resolve(process.argv[resumeAt + 1]!) : resolve(
  here,
  "../../.hitslop/v1-evidence/engine-placement",
  new Date().toISOString().replaceAll(":", "-"),
);
await mkdir(destination, { recursive: true });
let active: ReturnType<typeof Bun.spawn> | undefined;
process.on("SIGTERM", () => {
  active?.kill();
  process.exit(143);
});
const run = async (args: string[]) => {
  const p = Bun.spawn(args, { cwd: resolve(here, "../.."), stdout: "pipe", stderr: "pipe", env });
  active = p;
  const timeout = setTimeout(()=>p.kill(),240_000);
  const [stdout, stderr, exit] = await Promise.all([
    new Response(p.stdout).text(),
    new Response(p.stderr).text(),
    p.exited,
  ]);
  clearTimeout(timeout);
  if (exit) throw new Error(`${args.join(" ")}\n${stdout}\n${stderr}`);
  return stdout;
};
if (!resuming) {
const bin = (
  await run(["swift", "build", "--package-path", here, "-c", "release", "--show-bin-path"])
).trim();
await cp(join(here, "dist"), join(destination, "assets"), { recursive: true });
await cp(join(bin, "engine-placement"), join(destination, "harness"));
if (rust) {
  const rustBin = (await run(["/usr/bin/swift", "build", "--package-path", resolve(here,"../hitslop-core"), "-c", "release", "--show-bin-path"])).trim();
  await cp(join(rustBin,"rust-placement"), join(destination,"rust-harness"));
}
const version = await run(["sw_vers"]);
const source = await run(["git", "rev-parse", "HEAD"]);
const cpu = await run(["sysctl", "-n", "machdep.cpu.brand_string"]);
const hash = (bytes: Buffer) => createHash("sha256").update(bytes).digest("hex");
await Bun.write(
  join(destination, "environment.json"),
  JSON.stringify(
    {
      created: new Date().toISOString(),
      version,
      cpu,
      source,
      mode,
      adapter:
        "indexed IDs and confirmed move patches; atomic fork staging retained; periodic 200ms autosave parity asserted",
      rustHarnessHash: rust ? hash(await readFile(join(destination,"rust-harness"))) : undefined,
      harnessHash: hash(await readFile(join(destination, "harness"))),
      appHash: hash(await readFile(join(destination, "assets/app.js"))),
      swiftResolution: JSON.parse(await readFile(join(here, "Package.resolved"), "utf8")),
      wasmBaseline: "1.16.1",
      matchedCore: "1.16.2",
      seedHash: hash(await readFile(join(here, "dist/checklist-1000.snapshot"))),
      method:
        "Fresh process, release build, identical 40 rendered rows including final row; nonpersistent WebKit stores; two rAFs are a render opportunity proxy, not compositor paint. Host and identified WebContent footprints exclude GPU/network processes. Local relay measurements are separate.",
    },
    null,
    2,
  ),
);
if (rust) {
  await run([join(destination,"rust-harness"),join(destination,"assets"),join(destination,"integration.json"),"rust-core","1000","1","checklist","5","integration"]);
  console.log("Native WebView correctness gates passed", destination);
}
if (process.argv.includes("--integration-only")) process.exit(0);
}
else {
  const frozen = await Bun.file(join(destination,"environment.json")).json();
  if (frozen.mode !== mode || (rust && !frozen.rustHarnessHash)) throw new Error("Resume configuration differs from frozen run");
  const hash = (bytes: Buffer) => createHash("sha256").update(bytes).digest("hex");
  for (const [name, expected] of [["harness",frozen.harnessHash],["rust-harness",frozen.rustHarnessHash],["assets/app.js",frozen.appHash]]) {
    if (expected && hash(await readFile(join(destination,name))) !== expected) throw new Error(`Frozen artifact changed: ${name}`);
  }
}
const records: any[] = [];
if (resuming && await Bun.file(join(destination,"index.json")).exists()) {
  for (const entry of await Bun.file(join(destination,"index.json")).json()) if (!redo.has(entry.file)) records.push({...await Bun.file(join(destination,entry.file)).json(),trial:entry.trial,file:entry.file});
}
const trials = mode === "matrix" ? 5 : 1;
const stressTrials = rust ? 1 : trials;
const cells =
  mode === "matrix"
    ? [1000, 5000, 40000].flatMap((rows) => [1, 5, 10].map((windows) => ({ rows, windows })))
    : [{ rows: 1000, windows: 1 }];
const totalRuns = cells.reduce((n,c)=>n+(c.rows===40000?stressTrials:trials)*candidates.length,0);
await Bun.write(join(destination,"matrix-policy.json"),JSON.stringify({ordinaryTrials:trials,stressTrials,stressRows:40000,totalRuns},null,2)+"\n");
for (let trial = 0; trial < trials; trial++)
  for (const { rows, windows } of cells) {
    if (rows === 40000 && trial >= stressTrials) continue;

    // Rotate ordering each cell and trial; never run competing CPU workloads in parallel.
    const rotation = (trial + cells.findIndex((c) => c.rows === rows && c.windows === windows)) % 3;
    for (const candidate of [...candidates.slice(rotation), ...candidates.slice(0, rotation)]) {
      const filename = `${trial}-${rows}-${windows}-${candidate}.json`;
      if (records.some(r=>r.file===filename)) continue;
      const output = join(destination, filename);
      // Unindexed/interrupted attempts must never seed the next process with
      // partially edited SQLite state. Preserve them as evidence, then start fresh.
      const suffix = `.superseded-${Date.now()}`;
      for (const previous of [output,output.slice(0,-5)]) {
        try { await stat(previous); await rename(previous,previous+suffix); }
        catch (error:any) { if (error.code !== "ENOENT") throw error; }
      }
      const t = performance.now();
      let failed: string | undefined;
      try { await run([
        join(destination, candidate === "rust-core" ? "rust-harness" : "harness"),
        join(destination, "assets"),
        output,
        candidate,
        String(rows),
        String(windows),
        "checklist",
        mode === "matrix" ? "20" : "5",
      ]); } catch (error) {
        failed = String(error);
        await Bun.write(output, JSON.stringify({candidate,rows,windows,fixture:"checklist",failed:true,error:failed},null,2)+"\n");
      }
      const record = await Bun.file(output).json();
      records.push({ ...record, trial, file: filename });
      await Bun.write(
        join(destination, "index.json"),
        JSON.stringify(
          records.map(({ candidate, trial, windows, rows, file }) => ({
            candidate,
            trial,
            windows,
            rows,
            file,
          })),
          null,
          2,
        ),
      );
      console.log(
        `${records.length}/${totalRuns} ${failed ? "FAILED " : ""}${filename} ${(performance.now() - t).toFixed(0)}ms`,
      );
    }
  }
await Bun.write(join(destination, "summary.json"), JSON.stringify(summarize(records), null, 2));
await Bun.write(
  join(here, "../../.hitslop/v1-evidence/engine-placement/latest.txt"),
  destination + "\n",
);
console.log(destination);
function summarize(records: any[]) {
  const median = (a: number[]) => a.toSorted((a, b) => a - b)[Math.floor(a.length / 2)];
  return cells.map(({ rows, windows }) => ({
    rows,
    windows,
    candidates: Object.fromEntries(
      candidates.map((candidate) => {
        const all = records.filter(
          (r) => r.candidate === candidate && r.rows === rows && r.windows === windows,
        );
        const r = all.filter(x=>!x.failed);
        if (!r.length) return [candidate,{runs:all.length,failed:all.length,successfulRuns:0}];
        return [
          candidate,
          {
            runs: all.length, failed:all.length-r.length, successfulRuns:r.length,
            openMedianMS: median(r.map((x) => x.openMS)),
            memoryMedianMiB: median(r.map((x) => x.hostAndContentMiB)),
            scenarios: Object.fromEntries(
              ["checkbox", "typing", "move", "paste"].map((k) => [
                k,
                {
                  acceptedP95MedianMS: median(r.map((x) => x.results[k].accepted.p95)),
                  renderP95MedianMS: median(r.map((x) => x.results[k].renderOpportunity.p95)),
                  engineP95MedianMS: median(r.map((x) => x.results[k].engine.p95)),
                },
              ]),
            ),
          },
        ];
      }),
    ),
  }));
}
