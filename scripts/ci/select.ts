/** Event policy only; file ownership remains in verification-inputs.ts. */
import { run } from "../../packages/hitslop/src/cli/process";
import { repository } from "../lib/artifacts";
import { nativeGateInputs, sharedInputs, touchesCompatibility, type TierName } from "../lib/verification-inputs";
import { changedPaths } from "../lib/verification";

/** Required jobs exercise shipping boundaries. Browser/sync qualification never blocks a PR.
 * `nativeGate` is false for a pull request into master that changes nothing in
 * `nativeGateInputs`: its Swift/native tiers are deferred to the master push. */
export function ciJobs(tiers: TierName[], event: string | undefined, nativeGate = true) {
  const pick = (names: TierName[]) => tiers.filter(name => names.includes(name)).join(",");
  return {
    fast: pick(["compat", "tooling", "contracts", "types", "bun", "cli", "packed", "landing"]),
    native: nativeGate ? pick(["swift", "native"]) : "",
    nativeRust: pick(["rust"]),
    rust: pick(["rust"]),
    deferred: nativeGate ? "" : pick(["swift", "native"]),
    qualification: event === "schedule" || event === "workflow_dispatch" ? "browser,dev-sync,cli,packed" : "",
  };
}

/** Whether a pull request into master changes an input of the native gate. */
export function touchesNativeGate(paths: string[]) {
  return touchesCompatibility(paths) || paths.some(path => [...sharedInputs, ...nativeGateInputs].some(pattern => pattern.test(path)));
}

/** Full compatibility does not enable unrelated nightly checks. No usable diff means
 * full coverage, including scheduled/manual runs and a newly created push branch. */
export function ciCompatibility(env: Record<string, string | undefined>, paths?: string[]): "smoke" | "full" {
  if (env.HITSLOP_NIGHTLY === "1" || env.GITHUB_EVENT_NAME === "schedule" || env.GITHUB_EVENT_NAME === "workflow_dispatch"
    || env.GITHUB_BASE_REF?.startsWith("release/") || env.GITHUB_REF_NAME?.startsWith("release/")
    || !paths || touchesCompatibility(paths)) return "full";
  return "smoke";
}

export function verificationArgs(env: Record<string, string | undefined>): string[] {
  const args = ["--list", "--json", "--native"];
  const event = env.GITHUB_EVENT_NAME;
  if (event === "schedule" || event === "workflow_dispatch") return [...args, "--all"];
  if (event === "pull_request") {
    const base = env.GITHUB_BASE_REF;
    if (!base) throw new Error("A pull request requires its base branch");
    return [...args, ...(base.startsWith("release/") ? ["--all"] : ["--base", `origin/${base}`])];
  }
  if (event === "push") {
    const ref = env.GITHUB_REF_NAME;
    if (!ref) throw new Error("A push requires its branch");
    if (ref.startsWith("release/")) return [...args, "--all"];
    const before = env.CHANGE_BASE;
    if (!before || !/^[a-f0-9]{40}$/.test(before)) throw new Error("A push requires its previous commit");
    // A newly created branch has no previous commit to diff.
    return [...args, ...(before === "0".repeat(40) ? ["--all"] : ["--base", before])];
  }
  throw new Error(`Unsupported CI event: ${event}`);
}

if (import.meta.main) {
  const selection = JSON.parse(await run([process.execPath, "scripts/verify.ts", ...verificationArgs(process.env)], { cwd: repository }));
  const base = process.env.GITHUB_BASE_REF;
  const ref = process.env.GITHUB_EVENT_NAME === "pull_request" ? `origin/${base}` : process.env.CHANGE_BASE;
  const paths = ref && ref !== "0".repeat(40) ? (await changedPaths(repository, ref)).paths : undefined;
  const gate = process.env.GITHUB_EVENT_NAME !== "pull_request" || !base || base.startsWith("release/")
    || touchesNativeGate(paths ?? []);
  const compatibility = ciCompatibility(process.env, paths);
  console.log(JSON.stringify({ ...selection, compatibility, jobs: {
    ...ciJobs(selection.tiers.map((tier: { name: TierName }) => tier.name), process.env.GITHUB_EVENT_NAME, gate),
    compatibility,
  } }));
}
