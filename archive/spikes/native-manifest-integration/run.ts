import { mkdir, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
const out = resolve(import.meta.dir, "../../generated/native-manifest-integration");
const [label, ...command] = process.argv.slice(2);
if (!label || !command.length || !/^[a-z0-9-]+$/.test(label))
  throw new Error("run.ts label command ...");
await mkdir(join(out, "logs"), { recursive: true });
await writeFile(join(out, `logs/${label}.stdout`), "");
await writeFile(join(out, `logs/${label}.stderr`), "");
await writeFile(
  join(out, `logs/${label}.json`),
  JSON.stringify({ command, code: null, status: "running" }, null, 2),
);
const env = {
  ...process.env,
  PATH: `${process.env.HOME}/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin`,
};
const started = performance.now();
const child = Bun.spawn(command, {
  cwd: join(out, "workspace"),
  env,
  stdout: Bun.file(join(out, `logs/${label}.stdout`)),
  stderr: Bun.file(join(out, `logs/${label}.stderr`)),
});
const code = await child.exited;
await writeFile(
  join(out, `logs/${label}.json`),
  JSON.stringify({ command, code, seconds: (performance.now() - started) / 1000 }, null, 2),
);
console.log(`${label}: exit ${code}; logs in ${join(out, "logs")}`);
process.exit(code);
