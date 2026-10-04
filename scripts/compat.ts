// The compatibility corpus (tests/compat): packages and saved documents captured from a
// release candidate, replayed by every later build. A frozen entry is never edited,
// regenerated or deleted; see docs/testing.md#compatibility-corpus.
import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { repository } from "./templates";
import { strict as assert } from "node:assert";

export const corpus = join(repository, "tests/compat");
export const helper = resolve(
  process.env.HITSLOP_NATIVE_CLI ?? join(repository, "apps/apple/Packages/HitSlopApple/.build/debug/hitslop-native"),
);

/** `release.json`: what captured the entry, and whether it is permanent. */
export type Release = {
  release: string;
  frozen: boolean;
  commit: string;
  captured: string;
  /** The compatibility markers the entry's build wrote. */
  markers: { packageFormat: number; runtimeABI: number; storage: number; layout: number; protocol: number };
  inputs: string;
  producer: { coreBuildID: string; shell: string };
  files: Record<string, string>;
  templates: Record<string, string>;
  archives: Record<string, string>;
  toolchain: Record<string, string>;
  /** Page scenarios run with this clock, so date-dependent apps behave the same later. */
  clock: number;
  /** Saved storage shapes the documents cover, by document name. */
  storage: Record<string, { checkpointBytes: number; updates: number }>;
};
/** `expected/<name>.json`: the saved document as its release read it. */
export type Expected = {
  value: unknown;
  issues: unknown[];
  theme: { overrides: Record<string, string>; effective: Record<string, string> };
  attachments: { id: string; byteLength: number }[];
};
/** `scenarios/<name>.json`: a CLI edit replayed on the frozen document, and its result. */
export type Scenario = { ops: unknown[]; value: unknown; issues: unknown[] };
/** `pages/<name>.json`: an edit the old app makes in its own page, and the saved result
 * with page-minted row IDs replaced by `minted-N`. */
export type Page = { script: "contractTest" | "actions"; actions?: { selector: string; value: string; enter?: boolean }[]; value: unknown };
/** `cli/transcript.json`: `slop` commands as the entry's release ran them, and what they printed. */
export type Transcript = {
  document: string;
  commands: { args: string[]; code: number; stdout: unknown; outputHash?: string }[];
};

export async function releases(): Promise<{ name: string; root: string; release: Release }[]> {
  const found = [];
  for (const entry of await readdir(corpus, { withFileTypes: true }).catch(() => [])) {
    if (!entry.isDirectory()) continue;
    const root = join(corpus, entry.name);
    const release = JSON.parse(await readFile(join(root, "release.json"), "utf8")) as Release;
    found.push({ name: entry.name, root, release });
  }
  return found.sort((a, b) => a.name.localeCompare(b.name));
}

export async function documents(root: string): Promise<string[]> {
  return (await readdir(join(root, "documents")))
    .filter((name) => name.endsWith(".slop"))
    .map((name) => name.slice(0, -".slop".length))
    .sort();
}

export async function readJSON<T>(path: string): Promise<T | undefined> {
  return readFile(path, "utf8").then(
    (text) => JSON.parse(text) as T,
    (error) => {
      if (error.code === "ENOENT") return undefined;
      throw error;
    },
  );
}

/** Runs a `slop` command through this build's CLI and helper, returning its exit code and output. */
export async function slop(args: string[]) {
  const child = Bun.spawn([process.execPath, join(repository, "packages/cli/src/cli.ts"), ...args], {
    env: { ...process.env, HITSLOP_NATIVE_CLI: helper }, stdout: "pipe", stderr: "pipe",
  });
  const timeout = setTimeout(() => child.kill(), 120_000);
  try {
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { stdout, stderr, code };
  } finally {
    clearTimeout(timeout);
  }
}
export async function slopJSON(args: string[]): Promise<any> {
  const { stdout, stderr, code } = await slop(args);
  if (code) throw new Error(`slop ${args.join(" ")} failed (${code}): ${stderr.trim()}`);
  return JSON.parse(stdout);
}

/** What a later build must reproduce: the value and issues, not the version or sequence. */
export async function savedState(document: string): Promise<Expected> {
  const { state } = await slopJSON(["get", document, "--snapshot"]);
  const theme = await slopJSON(["theme", "get", document]);
  const attachments = await slopJSON(["attachments", "list", document]);
  return {
    value: state.value,
    issues: state.issues,
    theme: { overrides: parse(theme.overrides), effective: parse(theme.effective) },
    attachments: (attachments.attachments ?? attachments).map(({ id, byteLength }: any) => ({ id, byteLength })),
  };
}
const parse = (value: unknown) => (typeof value === "string" ? JSON.parse(value) : value);

/** Normalizes the parts of CLI output that name a session rather than a document. */
export function stable(output: unknown, args: readonly string[]): unknown {
  const omitSession = (value: any) => Object.fromEntries(Object.entries(value)
    .filter(([key]) => !["version", "epoch", "sequence"].includes(key)));
  if (output && typeof output === "object" && !Array.isArray(output)) {
    if (args[0] === "get" && args.includes("--snapshot")) {
      const frame = output as any;
      return { ...frame, ...(frame.state ? { state: omitSession(frame.state) } : {}) };
    }
    if (["apply", "batch", "import"].includes(args[0]!)) return omitSession(output);
  }
  return output;
}

/** Envelope metadata can grow; authored values, descriptors, palettes and IDs cannot. */
export function assertOutput(actual: unknown, expected: unknown, args: readonly string[], label = "CLI output") {
  const a = stable(actual, args) as any, e = stable(expected, args) as any;
  const envelope = (got: any, wanted: any) => {
    assert.ok(got && typeof got === "object" && !Array.isArray(got), label);
    for (const [key, value] of Object.entries(wanted)) {
      assert.ok(Object.hasOwn(got, key), `${label}: missing ${key}`);
      assert.deepEqual(got[key], value, `${label}: ${key}`);
    }
  };
  if (e && typeof e === "object" && !Array.isArray(e)) {
    if (args[0] === "get" && args.includes("--snapshot")) {
      const { state, ...rest } = e;
      envelope(a, rest);
      envelope(a.state, state);
      return;
    }
    if (["apply", "batch", "import", "theme"].includes(args[0]!)) return envelope(a, e);
  }
  assert.deepEqual(a, e, label);
}
