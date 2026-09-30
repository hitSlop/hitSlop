// Run sequentially: native UI tests and authoring subprocess deadlines are sensitive to load.
import { run } from "./harness";
const name = process.argv[2] ?? "combined";
const checks: [string, string[]][] = [
  ["swift-all", ["bun", "run", "swift:test"]],
  ["native", ["bun", "run", "test:native"]],
  ["render", ["bun", "run", "test:render", "--fixtures"]],
  ["crash", ["bun", "scripts/crash-matrix.ts"]],
  ["check", ["bun", "run", "check"]],
  ["typescript", ["bun", "run", "test"]],
  ["rust", ["cargo", "test", "--locked", "--workspace"]],
];
let failures = 0;
for (const [label, args] of checks) if ((await run(name, label, args)).code !== 0) failures++;
process.exitCode = failures ? 1 : 0;
