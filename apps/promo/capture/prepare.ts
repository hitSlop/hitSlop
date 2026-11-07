/** Create current writable copies through the CLI. Existing demo documents are left intact. */
import { mkdir, copyFile } from "node:fs/promises";
import { join } from "node:path";
const repo = join(import.meta.dir, "../../..");
const cli = join(repo, "packages/pop/src/cli/cli.ts");
async function run(...args: string[]) {
  const p = Bun.spawn(["bun", cli, ...args], { cwd: repo, stdout: "pipe", stderr: "inherit" });
  const output = await new Response(p.stdout).text();
  if (await p.exited) throw new Error(`pop ${args[0]} failed`);
  return output;
}
const catalog = JSON.parse(await run("templates"));
if (catalog.issues?.length) throw new Error(JSON.stringify(catalog.issues));
const slugs = [
  "soma-amp",
  "pocket-pod",
  "flashcards",
  "focus-timer",
  "baby-journal",
  "codex-pet",
  "koi-pond",
  "doodle-board",
  "wordle",
  "pixel-art",
  "pocket-sheet",
];
for (const slug of slugs) {
  const template = catalog.templates.find((t: { slug: string }) => t.slug === slug);
  if (!template) throw new Error(`Template missing: ${slug}. Build and register it first.`);
  const file = join(
    import.meta.dir,
    `../public/demo/${slug === "baby-journal" ? "little-days" : slug === "focus-timer" ? "pomodoro" : slug}.pop`,
  );
  if (!(await Bun.file(file).exists())) await run("create", "--from", slug, "--output", file);
  await run("inspect", file);
  const description = JSON.parse(await run("describe", file, "--json"));
  console.log(`${slug}: ${description.metadata.title}, writable .pop verified`);
}

await mkdir(join(import.meta.dir, "../public/brand"), { recursive: true });
for (const name of ["wordmark.webp", "mark.webp"])
  await copyFile(
    join(repo, "apps/landing/public/assets/brand", name),
    join(import.meta.dir, "../public/brand", name),
  );
