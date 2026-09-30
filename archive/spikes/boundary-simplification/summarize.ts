import { readFile, writeFile, readdir, mkdir, copyFile } from "node:fs/promises";
import { join } from "node:path";
import { homedir } from "node:os";
import { out, root, exists } from "./harness";

const retained = join(root, "archive/spikes/boundary-simplification/evidence");
await mkdir(retained, { recursive: true });
const baseline = JSON.parse(await readFile(join(out, "baseline.json"), "utf8"));
const checks = [];
for (const file of (await readdir(join(out, "logs"))).filter(f => f.endsWith(".json")).sort()) {
  const log = JSON.parse(await readFile(join(out, "logs", file), "utf8"));
  checks.push({ name: log.name, label: log.label, command: log.args, exit: log.code, seconds: log.seconds,
    tail: (log.stdout + log.stderr).slice(-2500) });
}
const measurements: Record<string, unknown> = {};
for (const candidate of ["A", "B", "C", "D", "shape", "validation", "sqlite-module", "session", "combined"]) {
  const folder = join(out, "evidence", candidate);
  if (!(await exists(folder))) continue;
  const values: Record<string, unknown> = {};
  for (const file of await readdir(folder)) {
    if (!file.endsWith(".json")) continue;
    values[file.replace(/^boundary-/, "").replace(/\.json$/, "")] = JSON.parse(await readFile(join(folder, file), "utf8"));
  }
  measurements[candidate] = values;
}
const patches = [];
for (const file of await readdir(join(root, "archive/spikes/boundary-simplification/patches")).catch(() => [])) {
  const text = await readFile(join(root, "archive/spikes/boundary-simplification/patches", file), "utf8");
  const lines = text.split("\n");
  patches.push({ name: file, added: lines.filter(l => l.startsWith("+") && !l.startsWith("+++")).length, removed: lines.filter(l => l.startsWith("-") && !l.startsWith("---")).length });
}
const harnessVerification: Record<string, unknown> = {};
for (const name of ["reproduction", "patches", "source-preservation"]) {
  const path = join(out, "evidence", `${name}.json`);
  if (await exists(path)) harnessVerification[name] = JSON.parse(await readFile(path, "utf8"));
}
const finalAcceptance = ["build", "check", "typescript", "rust", "swift-all", "native", "render", "crash"].map(label => {
  const final = checks.find(c => c.name === "combined" && c.label === `${label}-final`)
    ?? checks.find(c => c.name === "combined" && c.label === label);
  return { check: label, evidence: final?.label ?? null, passed: final?.exit === 0 };
});
await writeFile(join(retained, "results.json"), JSON.stringify({ recordedAt: new Date().toISOString(), baseline: { head: baseline.head, digest: baseline.digest, tools: baseline.tools }, finalAcceptance, checks, measurements, patches, harnessVerification }, (_key, value) => typeof value === "string" ? value.replaceAll(root, "<repo>").replaceAll(homedir(), "<home>") : value, 2) + "\n");
await copyFile(join(out, "baseline.json"), join(retained, "baseline.json"));
console.log(JSON.stringify({ checks: checks.map(({name,label,exit}) => ({name,label,exit})), measurements: Object.keys(measurements), patches }, null, 2));
