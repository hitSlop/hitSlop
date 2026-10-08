/** Event policy only; file ownership remains in verification-inputs.ts. */
import { run } from "../../packages/hitslop/src/cli/process";
import { repository } from "../lib/artifacts";

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
  console.log(await run([process.execPath, "scripts/verify.ts", ...verificationArgs(process.env)], { cwd: repository }));
}
