import { expect, test } from "bun:test";
import { exec } from "../../src/cli/process";

test("subprocesses inherit environment changes made by their caller", async () => {
  const prior = process.env.HITSLOP_PROCESS_TEST;
  process.env.HITSLOP_PROCESS_TEST = "current";
  try {
    const result = await exec([process.execPath, "-e", "console.log(process.env.HITSLOP_PROCESS_TEST)"]);
    expect(result.code).toBe(0);
    expect(result.stdout.trim()).toBe("current");
  } finally {
    if (prior === undefined) delete process.env.HITSLOP_PROCESS_TEST;
    else process.env.HITSLOP_PROCESS_TEST = prior;
  }
});
