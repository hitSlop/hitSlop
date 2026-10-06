/** Opt-in production-owner durability measurement over the active conformance fixture. */
const days = Number(process.env.HITSLOP_GROWTH_DAYS ?? 30);
if (!Number.isInteger(days) || days < 1 || days > 365) throw new Error("Days must be 1…365");
const git = Bun.spawn(["git", "rev-parse", "HEAD"], { stdout: "pipe" });
const commit = (await new Response(git.stdout).text()).trim();
if (await git.exited) throw new Error("Cannot identify benchmark commit");
const child = Bun.spawn([process.execPath, "run", "swift:test", "--filter", "DocumentGrowthTests"], {
  env: {
    ...process.env,
    HITSLOP_GROWTH: "1",
    HITSLOP_GROWTH_DAYS: String(days),
    HITSLOP_GROWTH_COMMIT: commit,
    HITSLOP_GROWTH_OUTPUT: process.env.HITSLOP_GROWTH_OUTPUT ??
      `${process.cwd()}/docs/evidence/document-growth-${new Date().toISOString().slice(0, 10)}.json`,
  },
  stdout: "inherit", stderr: "inherit",
});
process.exitCode = await child.exited;

export {};
