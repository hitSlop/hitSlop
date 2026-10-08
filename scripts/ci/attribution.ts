import { appendFile, readFile } from "node:fs/promises";
import { run } from "../lib/test-process";

// Match assistant identities, not vendor domains: human employees remain valid authors.
// A human named Claude is valid; the assistant's bare name needs a known bot address.
const assistantName = /^(?:claude (?:code|opus|sonnet|haiku)(?:\b.*)?|(?:openai )?codex|(?:github )?copilot(?: (?:coding )?agent)?|cursor(?: agent)?|chatgpt|gemini(?: cli| code assist)?|aider|devin(?: ai)?|openhands|opencode|amp(?: code)?)(?:\s*\[bot\])?$/i;
const assistantAddress = /^(?:noreply@anthropic\.com|(?:codex|agent)@openai\.com|(?:\d+\+)?(?:claude|claude-code|codex|copilot|copilot-swe-agent|cursor-agent|gemini-code-assist|devin-ai-integration)\[bot\]@users\.noreply\.github\.com)$/i;
const generatedAgent = /^(?:claude|anthropic|codex|openai|copilot|github copilot|cursor|chatgpt|gemini|aider|devin|openhands|opencode|amp|an? ai(?: assistant)?)(?:\b|$)/i;

export type Finding = { source: string; line: string };
/** Release fixes follow the same trusted attribution policy as development PRs. */
export function protectedPullBase(ref: string): boolean {
  return ref === "master" || /^release\/[^/]+$/.test(ref);
}
export function assistantIdentity(name: string, email: string): boolean {
  return assistantName.test(name.trim()) || assistantAddress.test(email.trim());
}

/** Only attribution-shaped lines are policy violations; tool names in prose are fine. */
export function attributionLines(text: string, source: string): Finding[] {
  return text.split(/\r?\n/).flatMap(line => {
    const plain = line.trim().replace(/\[([^\]]+)\]\([^)]*\)/g, "$1").replace(/[*_`]/g, "");
    const coauthor = /^co-authored-by:\s*(.*?)\s*<([^<>]+)>\s*$/i.exec(plain);
    const generated = /^(?:🤖\s*)?(?:generated|created|written|authored|assisted)\s+(?:with|by|using)\s+(.+)$/i.exec(plain);
    const forbidden = (coauthor && assistantIdentity(coauthor[1]!, coauthor[2]!))
      || (generated && (generatedAgent.test(generated[1]!) || plain.startsWith("🤖")))
      || /^claude-session:\s*https:\/\/claude\.ai\//i.test(plain);
    return forbidden ? [{ source, line }] : [];
  });
}

/** Read objects only: none of the incoming tree, hooks or build scripts is executed. */
export async function inspectCommits(cwd: string, base: string, head: string): Promise<Finding[]> {
  for (const sha of [base, head]) {
    if (!/^[a-f0-9]{40}$/.test(sha)) throw new Error("Expected a complete Git commit SHA");
    await run(["git", "cat-file", "-e", `${sha}^{commit}`], { cwd });
  }
  const commits = (await run(["git", "rev-list", `${base}..${head}`], { cwd })).trim().split("\n").filter(Boolean);
  const findings: Finding[] = [];
  for (const sha of commits) {
    const raw = await run(["git", "show", "-s", "--no-show-signature", "--format=%an%x00%ae%x00%cn%x00%ce%x00%B", sha], { cwd });
    const [author, authorEmail, committer, committerEmail, ...message] = raw.split("\0");
    for (const [kind, name, email] of [["author", author, authorEmail], ["committer", committer, committerEmail]]) {
      if (name === undefined || email === undefined) throw new Error("Incomplete Git identity");
      if (assistantIdentity(name, email)) findings.push({ source: `${sha} ${kind}`, line: `${name} <${email}>` });
    }
    findings.push(...attributionLines(message.join("\0"), sha));
  }
  return findings;
}

type Pull = {
  number: number; state: string; title: string; body: string | null;
  head: { sha: string }; base: { sha: string; ref: string };
};

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing ${name}`);
  return value;
}

async function checkPullRequest() {
  const event = JSON.parse(await readFile(requiredEnv("GITHUB_EVENT_PATH"), "utf8")) as { number: number };
  if (!Number.isSafeInteger(event.number) || event.number < 1) throw new Error("Invalid pull request number");
  const repository = requiredEnv("GITHUB_REPOSITORY");
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error("Invalid repository");
  const token = requiredEnv("GH_TOKEN");
  const api = async <T>(path: string, body?: unknown): Promise<T> => {
    const response = await fetch(`https://api.github.com/repos/${repository}/${path}`, {
      method: body === undefined ? "GET" : "POST",
      headers: { Authorization: `Bearer ${token}`, Accept: "application/vnd.github+json", "Content-Type": "application/json", "X-GitHub-Api-Version": "2022-11-28" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(30_000),
    });
    if (!response.ok) throw new Error(`GitHub API ${response.status} for ${path}`);
    return await response.json() as T;
  };
  const pullPath = `pulls/${event.number}`;
  const pull = await api<Pull>(pullPath);
  if (pull.state !== "open" || !protectedPullBase(pull.base.ref)) return;
  if (![pull.head.sha, pull.base.sha].every(sha => /^[a-f0-9]{40}$/.test(sha))) throw new Error("Invalid PR commit SHA");
  const status = (state: "pending" | "success" | "failure" | "error", description: string) => api(`statuses/${pull.head.sha}`, {
    state, context: "Attribution", description,
    target_url: `https://github.com/${repository}/actions/runs/${requiredEnv("GITHUB_RUN_ID")}`,
  });
  await status("pending", "Checking incoming commits and PR metadata for AI attribution");
  try {
    const cwd = process.cwd();
    // Public Git objects need no credentials. Use the base repository's PR ref, never a fork URL.
    await run(["git", "-c", "credential.helper=", "fetch", "--no-tags", "--no-write-fetch-head", `https://github.com/${repository}.git`,
      `+refs/pull/${event.number}/head:refs/attribution/head`, pull.base.sha], { cwd, timeout: 120_000 });
    const fetched = (await run(["git", "rev-parse", "refs/attribution/head"], { cwd })).trim();
    if (fetched !== pull.head.sha) throw new Error("PR changed while fetching; rerun against the latest revision");
    const findings = [
      ...attributionLines(pull.title, "PR title"),
      ...attributionLines(pull.body ?? "", "PR description"),
      ...await inspectCommits(cwd, pull.base.sha, pull.head.sha),
    ];
    const latest = await api<Pull>(pullPath);
    if (latest.state !== "open" || latest.base.ref !== pull.base.ref || latest.base.sha !== pull.base.sha
      || latest.head.sha !== pull.head.sha || latest.title !== pull.title || latest.body !== pull.body)
      throw new Error("PR changed during validation; rerun against the latest revision");
    const summary = findings.length
      ? "Remove AI attribution from the listed commits or PR metadata. Amend/rebase the affected commits and push with --force-with-lease. Human co-authors are welcome.\n\n"
        + findings.map(f => `${f.source}: ${f.line}`).join("\n")
      : "All incoming commits and PR metadata are free of recognized AI attribution.";
    // Escape untrusted metadata instead of turning it into workflow commands or Markdown.
    console.log(JSON.stringify(summary));
    const summaryPath = process.env.GITHUB_STEP_SUMMARY;
    if (summaryPath) await appendFile(summaryPath, `<pre>${summary.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")}</pre>\n`);
    await status(findings.length ? "failure" : "success", findings.length ? "Remove AI attribution; see the workflow summary" : "No recognized AI attribution");
    if (findings.length) process.exitCode = 1;
  } catch (error) {
    await status("error", "Attribution inspection failed; inspect the workflow log and rerun");
    throw error;
  }
}

if (import.meta.main) await checkPullRequest();
