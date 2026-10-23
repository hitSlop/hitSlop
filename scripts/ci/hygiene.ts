/** Repository checks for what ships to the public, run by the `types` tier
 * (`bun run verify types`): no placeholder in tracked source or docs, one iPhone download
 * address everywhere, and a landing template catalog that matches the active templates. */
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { run } from "../../packages/hitslop/src/cli/process";
import { repository } from "../lib/artifacts";
import { discoverTemplates } from "../templates/discover";
import { SITE_LINKS } from "../../apps/landing/src/constants";

const problems: string[] = [];

/** A placeholder address that must be replaced before it can ship. Spelled in two parts so
 * this file does not contain it. */
const placeholder = "REPLACE" + "_ME";

async function placeholders() {
  const listing = await run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], { cwd: repository });
  for (const path of listing.split("\0").filter(Boolean)) {
    const bytes = await readFile(join(repository, path)).catch(() => undefined);
    if (!bytes || bytes.length > 4 * 1024 * 1024 || bytes.includes(0)) continue;
    const line = bytes.toString("utf8").split("\n").findIndex(text => text.includes(placeholder));
    if (line >= 0) problems.push(`${path}:${line + 1} contains ${placeholder}. Replace it with the real value before it ships.`);
  }
}

/** The iPhone download address is one constant for the landing site, mirrored by the README link
 * and the app's links; the site's own relative addresses are served from hitslop.com. */
async function iosDownload() {
  const expected = SITE_LINKS.iosDownload.startsWith("/") ? `https://hitslop.com${SITE_LINKS.iosDownload}` : SITE_LINKS.iosDownload;
  const mirrors: [string, RegExp][] = [
    ["README.md", /^\[ios-download\]:\s*(\S+)/m],
    ["apps/apple/Packages/HitSlopApple/Sources/HitSlopCore/SlopLinks.swift", /iosDownload\s*=\s*URL\(string:\s*"([^"]+)"/],
  ];
  for (const [path, pattern] of mirrors) {
    const found = pattern.exec(await readFile(join(repository, path), "utf8"))?.[1];
    if (found !== expected) problems.push(`${path}: the iPhone download address is ${found ?? "missing"}, not ${expected} (SITE_LINKS.iosDownload in apps/landing/src/constants.ts).`);
  }
}

/** The template wall's data is generated and committed (the website build never builds
 * templates), so it drifts when a template is added, removed or recategorized. */
async function catalog() {
  const regenerate = "Regenerate it: bun run build:templates && bun apps/landing/scripts/templates.ts, then commit apps/landing/src/data/templates.json and public/assets/templates.";
  const listed = JSON.parse(await readFile(join(repository, "apps/landing/src/data/templates.json"), "utf8")) as { slug: string; categories: string[] }[];
  const bySlug = new Map(listed.map(entry => [entry.slug, entry.categories]));
  const templates = await discoverTemplates();
  const active = new Set(templates.map(({ slug }) => slug));
  const missing = [...active].filter(slug => !bySlug.has(slug));
  const stale = [...bySlug.keys()].filter(slug => !active.has(slug));
  const moved: string[] = [];
  for (const { slug, source } of templates) {
    const categories = bySlug.get(slug);
    if (!categories) continue;
    const declared = ((await import(join(source, "slop.ts"))).default as { categories: string[] }).categories;
    if (JSON.stringify(declared) !== JSON.stringify(categories)) moved.push(`${slug} (${categories.join(", ")} -> ${declared.join(", ")})`);
  }
  if (missing.length || stale.length || moved.length)
    problems.push([
      "apps/landing/src/data/templates.json no longer matches the active templates (slops/bundled.json and each slop.ts).",
      ...(missing.length ? [`  missing ${missing.length}: ${missing.join(", ")}`] : []),
      ...(stale.length ? [`  no longer a template ${stale.length}: ${stale.join(", ")}`] : []),
      ...(moved.length ? [`  categories changed ${moved.length}: ${moved.join("; ")}`] : []),
      `  ${regenerate}`,
    ].join("\n"));
}

await Promise.all([placeholders(), iosDownload(), catalog()]);
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("No placeholders, one iPhone download address, and the landing template catalog matches the templates.");
