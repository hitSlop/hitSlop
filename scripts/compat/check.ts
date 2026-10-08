import { repository } from "../lib/artifacts";
import { releases } from "./corpus";
import { verifyCorpus } from "./integrity";
import { run } from "../../packages/hitslop/src/cli/process";

/** Frozen compatibility corpus entries (tests/compat) are permanent: against the
 * protected branch, a change may only add files to them. Git history is the authority;
 * hashes stored beside the files could be edited with them. CI names the base
 * (`HITSLOP_COMPAT_BASE`): the pull request's target, or the commit a push replaced. */
export async function assertFrozenCorpus(
  root = repository,
  base = process.env.HITSLOP_COMPAT_BASE || "origin/master",
): Promise<void> {
  const resolved = Bun.spawnSync(["git", "rev-parse", "--verify", "--quiet", `${base}^{commit}`], { cwd: root });
  if (resolved.exitCode !== 0) {
    if (process.env.CI) throw new Error(`Cannot compare tests/compat with ${base}`);
    process.stdout.write(`! tests/compat not compared: ${base} is unavailable\n`);
    return;
  }
  const mergeBase = (await run(["git", "merge-base", "HEAD", base], { cwd: root })).trim();
  const listing = Bun.spawnSync(["git", "ls-tree", "--name-only", `${mergeBase}:tests/compat`], { cwd: root, stderr: "ignore" });
  const entries = listing.exitCode === 0 ? listing.stdout.toString().split("\n").filter(Boolean) : [];
  const changed: string[] = [];
  for (const entry of entries) {
    const release = Bun.spawnSync(["git", "show", `${mergeBase}:tests/compat/${entry}/release.json`], { cwd: root });
    if (release.exitCode !== 0 || JSON.parse(release.stdout.toString()).frozen !== true) continue;
    // Committed and uncommitted changes since the base; additions are the only allowed kind.
    const diff = await run(["git", "diff", "--name-status", "--no-renames", mergeBase, "--", `tests/compat/${entry}`], { cwd: root });
    changed.push(...diff.split("\n").filter((line) => line && !line.startsWith("A\t")));
  }
  if (changed.length)
    throw new Error(`Frozen compatibility corpus entries changed:\n${changed.map((line) => `  - ${line}`).join("\n")}`);
}

if (import.meta.main) {
  await assertFrozenCorpus();
  for (const entry of await releases()) await verifyCorpus(entry.root, entry.release);
  console.log("✓ compatibility corpus unchanged and complete");
}
