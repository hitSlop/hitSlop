// The compatibility corpus (tests/compat): packages and saved documents captured from a
// release candidate, replayed by every later build. A frozen entry is never edited,
// regenerated or deleted; see docs/testing.md#compatibility-corpus.
import { repository } from "../lib/artifacts";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { debugHelper } from "../lib/native";
import { exec } from "../../packages/hitslop/src/cli/process";
import { strict as assert } from "node:assert";
import type { AttachmentInfo } from "../../packages/hitslop/src/wire/engine.generated";

export const corpus = join(repository, "tests/compat");
export const helper = resolve(process.env.HITSLOP_NATIVE_CLI ?? debugHelper);
/** The checkout engine, independent of the selected renderer. */
export const documentEngine = () => resolve(process.env.HITSLOP_ENGINE ?? `target/${process.env.HITSLOP_CARGO_PROFILE || "release"}/slop-engine`);

/** `release.json`: what captured the entry, and whether it is permanent. */
export type Release = {
  release: string;
  frozen: boolean;
  commit: string;
  captured: string;
  /** The compatibility markers the entry's build wrote. */
  markers: { packageFormat: number; runtimeABI: number; storage: number; layout: number; protocol: number };
  inputs: string;
  producer: { coreBuildID: string; shell: string; runner?: Record<string, string> };
  files: Record<string, string>;
  acceptance?: Record<string, string>;
  templates: Record<string, string>;
  /** The candidate writer: the darwin-arm64 engine that wrote the entry's documents, kept at
   * `engine/darwin-arm64/slop-engine` so later builds can run it (`compat_writers.rs`). */
  writer: { buildId: string; commit: string; sha256: string };
  toolchain: Record<string, string>;
  /** Page scenarios run with this clock, so date-dependent apps behave the same later. */
  clock: number;
  /** Saved storage shapes the documents cover, by document name. */
  storage: Record<string, { checkpointBytes: number; updates: number }>;
};
/** `expected/<name>.json`: the saved document as its release read it. */
export type Expected = {
  value: unknown;
  theme: Pick<ThemeState, "overrides" | "effective">;
  attachments: AttachmentInfo[];
};
type ThemeState = { overrides: Record<string,string>; effective: Record<string,string> };
/** `scenarios/<name>.json`: a CLI edit replayed on the frozen document, and its result. */
export type Scenario = { ops: unknown[]; value: unknown };
/** `pages/<name>.json`: an edit the old app makes in its own page, and the saved result
 * with page-minted row IDs replaced by `minted-N`. */
export type Page = { script: "contractTest" | "actions"; actions?: ({ selector: string; value: string; enter?: boolean } | { selector: string; click: true } | { selector: string; focus: true })[]; value: unknown };
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

/** The entries the app-level replays (native CLI/helper and Swift page) run on an ordinary
 * change: the newest entry, and the newest entry for each set of format markers, since only
 * a marker change makes an older file different to read. Nightly, release and a required
 * release entry (`HITSLOP_COMPAT_RELEASE`) replay all. Rust replays every entry, always. */
export function replayedEntries<T extends { name: string; release: Release }>(entries: T[], all: boolean): T[] {
  if (all || entries.length <= 1) return entries;
  const newest = (list: T[]) => list.reduce((a, b) => (b.release.captured > a.release.captured ? b : a));
  const byMarkers = new Map<string, T[]>();
  for (const entry of entries) {
    const key = JSON.stringify(entry.release.markers);
    byMarkers.set(key, [...(byMarkers.get(key) ?? []), entry]);
  }
  const chosen = new Set([newest(entries), ...[...byMarkers.values()].map(newest)]);
  return entries.filter((entry) => chosen.has(entry));
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
export function slop(args: string[]) {
  return exec([process.execPath, join(repository, "packages/hitslop/src/cli/cli.ts"), ...args], {
    env: { ...process.env, HITSLOP_NATIVE_CLI: helper },
    timeout: 120_000,
  });
}
export async function slopJSON(args: string[]): Promise<any> {
  const { stdout, stderr, code } = await slop(args);
  if (code) throw new Error(`slop ${args.join(" ")} failed (${code}): ${stderr.trim()}`);
  return JSON.parse(stdout);
}

/** What a later build must reproduce: the value, not the version. */
export async function savedState(document: string): Promise<Expected> {
  const state = await slopJSON(["get", document, "--snapshot"]);
  const theme: ThemeState = await slopJSON(["theme", "get", document]);
  const attachments: AttachmentInfo[] = await slopJSON(["attachments", "list", document]);
  return { value: state.value, theme: { overrides: theme.overrides, effective: theme.effective }, attachments };
}
