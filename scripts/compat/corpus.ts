// The compatibility corpus (tests/compat): packages and saved documents captured from a
// release candidate, replayed by every later build. A frozen entry is never edited,
// regenerated or deleted; see docs/testing.md#compatibility-corpus.
import { repository } from "../lib/artifacts";
import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { exec } from "../../packages/hitslop/src/cli/process";
import type { AttachmentInfo } from "../../packages/hitslop/src/wire/engine.generated";

export const corpus = join(repository, "tests/compat");
/** The checkout engine. */
export const documentEngine = () =>
  resolve(
    process.env.HITSLOP_ENGINE ??
      `target/${process.env.HITSLOP_CARGO_PROFILE || "release"}/slop-engine`,
  );

/** `release.json`: producing inputs, saved artifacts, storage-schema hashes and open-limit
 * directions. Raw acceptance vectors freeze reader behavior; source code is not frozen. */
export type Release = {
  release: string;
  frozen: boolean;
  commit: string;
  captured: string;
  /** The compatibility markers the entry's build wrote. */
  markers: {
    packageFormat: number;
    runtimeABI: number;
    storage: number;
    layout: number;
    protocol: number;
  };
  sourceInputs: string;
  producer: { coreBuildID: string; shell: string; runner?: Record<string, string> };
  files: Record<string, string>;
  /** Physical storage definitions are immutable; later layouts add migrations. */
  storageSchemas: Record<string, string>;
  /** Every bound used when opening, reading or saving stored data. Values are decimal
   * u64 strings so capture never rounds them through a JavaScript number. */
  limits: { name: string; value: string; kind: "Max" | "Min" | "Exact" }[];
  templates: Record<string, string>;
  /** App identity computed by the producing reader, independently of file hashes. */
  templateAppDigests: Record<string, string>;
  /** The shipped darwin-arm64 writer, resolved by hash outside the corpus. */
  writer: { version: string; platform: "darwin-arm64"; sha256: string; buildId: string };
  toolchain: Record<string, string>;
  /** Page scenarios run with this clock, so date-dependent apps behave the same later. */
  clock: number;
  /** Saved storage shapes the documents cover, by document name. */
  storage: Record<string, { checkpointBytes: number; updates: number }>;
};
/** `expected/<name>.json`: the saved document as its release read it. */
export type Expected = {
  appDigest: string;
  value: unknown;
  theme: Pick<ThemeState, "overrides" | "effective">;
  attachments: AttachmentInfo[];
};
export type ThemeState = { overrides: Record<string, string>; effective: Record<string, string> };
/** `exports/<name>.json`: rendered content produced by the captured build. The PNG digest
 * covers decoded row-major RGBA8 pixels, excluding encoder metadata. PDF text is PDFKit's
 * page text, with its whitespace preserved. */
export type ExportGolden = {
  png: { width: number; height: number; rgbaSha256: string };
  pdf: { pages: { width: number; height: number; text: string }[] };
};
/** `themes/<name>.json`: ordered theme edits and the palettes to replay. */
export type ThemeScenario = {
  steps: { values?: Record<string, string>; reset?: string; state: ThemeState }[];
};
/** `commands/<name>.json`: independent probes, each starting from the saved document.
 * Stored evaluation replies and owner call refusals are distinct protocol boundaries. */
export type CommandProbe = {
  name: string;
  args: unknown;
  channel: "evaluate" | "call";
  expected: { ok: boolean; refused?: boolean; code?: string; errorIncludes?: string };
  now: number;
  seed: number[];
  evaluated: unknown;
  value: unknown;
};
/** `scenarios/<name>.json`: a CLI edit replayed on the frozen document, and its result. */
export type Scenario = { ops: unknown[]; value: unknown };
/** `pages/<name>.json`: an edit the old app makes in its own page, and the saved result
 * with page-minted row IDs replaced by `minted-N`. */
export type Page = { script: "contractTest"; value: unknown };
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

export type CompatibilityMode = "smoke" | "full";

/** Standalone replay is full. The verification runner explicitly chooses smoke when
 * appropriate; nightly and required-release replay cannot inherit a smoke restriction. */
export function compatibilityMode(env: Record<string, string | undefined>): CompatibilityMode {
  const mode = env.HITSLOP_COMPAT_MODE;
  if (mode && mode !== "smoke" && mode !== "full")
    throw new Error(`Invalid HITSLOP_COMPAT_MODE: ${mode}`);
  return env.HITSLOP_NIGHTLY === "1" || env.HITSLOP_COMPAT_RELEASE || mode !== "smoke"
    ? "full"
    : "smoke";
}

/** Smoke samples recent entries from each marker generation; it does not establish
 * equivalence between their apps or saved scenarios. Full replay retains every entry.
 * Rust replays every entry in both modes. */
export function replayedEntries<T extends { name: string; release: Release }>(
  entries: T[],
  all: boolean,
): T[] {
  if (all || entries.length <= 1) return entries;
  const newest = (list: T[]) =>
    list.reduce((a, b) => (b.release.captured > a.release.captured ? b : a));
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

/** Runs a `slop` command through this build's CLI and engine, returning its exit code and output. */
export function slop(args: string[]) {
  return exec([process.execPath, join(repository, "packages/hitslop/src/cli/cli.ts"), ...args], {
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
  const { appDigest } = await slopJSON(["inspect", document]);
  const state = await slopJSON(["get", document, "--snapshot"]);
  const theme: ThemeState = await slopJSON(["theme", "get", document]);
  const attachments: AttachmentInfo[] = await slopJSON(["attachments", "list", document]);
  return {
    appDigest,
    value: state.value,
    theme: { overrides: theme.overrides, effective: theme.effective },
    attachments,
  };
}
