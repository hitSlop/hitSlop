import { exec } from "../../scripts/lib/test-process";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { repository } from "../../scripts/lib/artifacts";

async function registry(override?: string) {
  const { stdout: out, stderr: error, code } = await exec([process.execPath, "-e", `import {useTestRegistry} from ${JSON.stringify(join(repository, "scripts/lib/artifacts.ts"))}; useTestRegistry(); console.log(process.env.HITSLOP_TEST_REGISTRY)`], {
    env: { ...process.env, HITSLOP_TEST_REGISTRY: override },
  });
  expect(error).toBe("");
  expect(code).toBe(0);
  return out.trim();
}

test("independent test runs never share a default writer registry", async () => {
  const [first, second] = await Promise.all([registry(), registry()]);
  expect(first).not.toBe(second);
}, 30_000);
test("test runs preserve an explicitly supplied writer registry", async () => {
  expect(await registry("/tmp/hitslop-explicit-registry")).toBe("/tmp/hitslop-explicit-registry");
}, 30_000);
