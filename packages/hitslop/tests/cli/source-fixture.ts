import { rename, writeFile } from "node:fs/promises";
import { exists } from "../../src/cli/fs";
import { stageProject } from "../../src/cli/build";
import { join } from "node:path";

/** Replaces fields of a copied project's `slop.ts` with TypeScript expressions, which may
 * read the authored declaration as `slop`. The authored file moves to `authored.ts`;
 * `statements` run first. */
export async function overrideSlop(source: string, fields: Record<string, string>, statements = "") {
  if (!(await exists(join(source, "authored.ts")))) await rename(join(source, "slop.ts"), join(source, "authored.ts"));
  const keys = new Set(["slug", "title", "description", "author", "categories", "document", "initial", "window", "theme", "view", "export", "icon", "commands", "artwork", ...Object.keys(fields)]);
  const overrides = [...keys].map(key => `${key}: ${fields[key] ?? `slop.${key}`}`).join(", ");
  await writeFile(
    join(source, "slop.ts"),
    `import slop from "./authored";\nimport { defineSlop } from "hitslop";\n${statements}\nexport default defineSlop({ ${overrides} });\n`,
  );
}

/** A project's stage at `output`: what the build compiles and evaluates, before the engine
 * packs it. */
export async function stage(source: string, output: string) {
  await stageProject(source, output);
  return output;
}
