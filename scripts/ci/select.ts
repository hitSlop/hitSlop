/** Event policy only; file ownership remains in verification-inputs.ts. */
import { run } from "../../packages/hitslop/src/cli/process";
import { repository } from "../lib/artifacts";
import { affectedTiers, nativeGateInputs, sharedInputs, sharingGateInputs, touchesCompatibility, type TierName } from "../lib/verification-inputs";
import { changedPaths } from "../lib/verification";

/** Required jobs exercise shipping boundaries. Browser qualification never blocks a PR; the
 * sharing relay's qualification (`sharing` in `main`) runs on a pull request only when it
 * changes code that relay, the clients or the browser host share.
 * `nativeGate` is false for a pull request into master that changes nothing in
 * `nativeGateInputs`: its Swift/native tiers are deferred to the master push. */
export function ciJobs(tiers: TierName[], event: string | undefined, nativeGate = true, appBuild = false) {
  const pick = (names: TierName[]) => tiers.filter(name => names.includes(name)).join(",");
  return {
    fast: pick(["compat", "tooling", "contracts", "types", "bun", "cli", "render", "packed", "landing"]),
    app: appBuild || event === "schedule" || event === "workflow_dispatch" ? "app" : "",
    native: nativeGate ? pick(["swift", "native"]) : "",
    nativeRust: pick(["rust"]),
    rust: pick(["rust"]),
    deferred: nativeGate ? "" : pick(["swift", "native"]),
    qualification: event === "schedule" || event === "workflow_dispatch" ? "browser,cli,packed" : "",
  };
}

/** Whether a pull request into master changes an input of the native gate. */
export function touchesNativeGate(paths: string[]) {
  return touchesCompatibility(paths) || paths.some(path => [...sharedInputs, ...nativeGateInputs].some(pattern => pattern.test(path)));
}

/** Whether a pull request changes live-sharing code: the relay, its clients and the core
 * and browser host they share. Master pushes qualify it in sharing.yml. */
export function touchesSharing(paths: string[]) {
  return paths.some(path => sharingGateInputs.some(pattern => pattern.test(path)));
}

export function verificationArgs(env: Record<string, string | undefined>): string[] {
  const args = ["--list", "--json", "--native"];
  const event = env.GITHUB_EVENT_NAME;
  if (event === "schedule" || event === "workflow_dispatch") return [...args, "--all"];
  if (event === "pull_request") {
    const base = env.GITHUB_BASE_REF;
    if (!base) throw new Error("A pull request requires its base branch");
    return [...args, ...(base.startsWith("hotfix/") ? ["--all"] : ["--base", `origin/${base}`])];
  }
  if (event === "push") {
    const ref = env.GITHUB_REF_NAME;
    if (!ref) throw new Error("A push requires its branch");
    if (ref.startsWith("hotfix/")) return [...args, "--all"];
    const before = env.CHANGE_BASE;
    if (!before || !/^[a-f0-9]{40}$/.test(before)) throw new Error("A push requires its previous commit");
    // A newly created branch has no previous commit to diff.
    return [...args, ...(before === "0".repeat(40) ? ["--all"] : ["--base", before])];
  }
  throw new Error(`Unsupported CI event: ${event}`);
}

if (import.meta.main) {
  const args = verificationArgs(process.env);
  const selection = JSON.parse(await run([process.execPath, "scripts/verify.ts", ...args], { cwd: repository }));
  const base = process.env.GITHUB_BASE_REF;
  const ref = process.env.GITHUB_EVENT_NAME === "pull_request" ? `origin/${base}` : process.env.CHANGE_BASE;
  const paths = ref && ref !== "0".repeat(40) ? (await changedPaths(repository, ref)).paths : undefined;
  const gate = process.env.GITHUB_EVENT_NAME !== "pull_request" || !base || base.startsWith("hotfix/")
    || touchesNativeGate(paths ?? []);
  console.log(JSON.stringify({ ...selection, jobs: {
    ...ciJobs(selection.tiers.map((tier: { name: TierName }) => tier.name), process.env.GITHUB_EVENT_NAME, gate,
      args.includes("--all") || affectedTiers(paths ?? [], ["app"]).length > 0),
    sharing: process.env.GITHUB_EVENT_NAME === "pull_request" && touchesSharing(paths ?? []) ? "sharing" : "",
  } }));
}
