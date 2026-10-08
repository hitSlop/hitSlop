import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
const source = join(import.meta.dir, "Sources/SpikeCore/Replica.swift");
const original = await readFile(source, "utf8");
const marker = "let staged = doc.fork()";
if (!original.includes(marker)) throw new Error("Atomic staging mutation target changed");
const destination = join(import.meta.dir, "../../.hitslop/v1-evidence/engine-placement");
await mkdir(destination, { recursive: true });
try {
  await writeFile(
    source,
    original.replace(marker, "let staged = doc"),
  );
  const child = Bun.spawn(
    [
      "swift",
      "test",
      "--package-path",
      import.meta.dir,
      "-c",
      "release",
      "--filter",
      "batchRejectsWithoutChangingStateOrVersion",
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
  const [out, err, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  await writeFile(join(destination, "atomic-sensitivity.log"), out + err);
  if (code === 0 || !out.includes("Expectation failed"))
    throw new Error("Owner test did not catch the intended partial-edit failure; inspect log");
  console.log("Atomicity mutation caught: a rejected batch changed state/version.");
} finally {
  await writeFile(source, original);
}
