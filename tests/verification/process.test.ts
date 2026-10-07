import { expect, test } from "bun:test";
import { testProcess, exec } from "../../scripts/lib/test-process";

test("test processes drain both streams and preserve an early failure", async () => {
  const result = await exec([process.execPath, "-e", 'console.log("o".repeat(100000)); console.error("failure"); process.exitCode=7;']);
  expect(result.code).toBe(7);
  expect(result.stdout.trim()).toHaveLength(100000);
  expect(result.stderr).toContain("failure");
}, 30_000);

test("a timed-out process is awaited and reports its deadline", async () => {
  await expect(exec([process.execPath, "-e", 'console.error("waiting for input"); setInterval(()=>{},1000);'], { timeout: 1_000, grace: 100 })).rejects.toThrow("timed out after 1000ms");
}, 30_000);

test("cancellation reaps a process group even when its descendant ignores termination", async () => {
  const controller = new AbortController();
  const script = 'process.on("SIGTERM",()=>{}); console.log("ready"); setInterval(()=>{},1000);';
  const parent = `Bun.spawn([process.execPath,"-e",${JSON.stringify(script)}], {stdout:"inherit",stderr:"inherit"}); setInterval(()=>{},1000);`;
  let output = "";
  const child = testProcess([process.execPath, "-e", parent], {
    signal: controller.signal, grace: 100, timeout: 20_000,
    onOutput: text => { output += text; if (output.includes("ready")) controller.abort(); },
  });
  const result = await child.output;
  expect(result.aborted).toBe(true);
  expect(result.timedOut).toBe(false);
  expect(result.stdout).toContain("ready");
}, 30_000);

test("subprocess output preserves UTF-8 characters across pipe chunks", async () => {
  const expected = "🦊é".repeat(100_000);
  const result = await exec([process.execPath, "-e", 'process.stdout.write("🦊é".repeat(100000));']);
  expect(result.stdout === expected).toBe(true);
}, 30_000);
