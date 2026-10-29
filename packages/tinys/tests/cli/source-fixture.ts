import { rename, writeFile } from "node:fs/promises";
import { exists } from "../../src/cli/fs";
import { stageProject } from "../../src/cli/build";
import { join } from "node:path";

/** Replaces fields of a copied project's `tiny.ts` with TypeScript expressions, which may
 * read the authored declaration as `tiny`. The authored file moves to `authored.ts`;
 * `statements` run first. */
export async function overrideTiny(source: string, fields: Record<string, string>, statements = "") {
  if (!(await exists(join(source, "authored.ts")))) await rename(join(source, "tiny.ts"), join(source, "authored.ts"));
  const keys = new Set(["slug", "title", "description", "author", "categories", "window", "theme", "artwork", ...Object.keys(fields)]);
  const overrides = [...keys].map(key => `${key}: ${fields[key] ?? `tiny.${key}`}`).join(", ");
  await writeFile(
    join(source, "tiny.ts"),
    `import tiny from "./authored";\nimport { defineTiny } from "@hitslop/tinys";\n${statements}\nexport default defineTiny({ ${overrides} });\n`,
  );
}

/** A project's stage at `output`: what the build compiles and evaluates, before the engine
 * packs it. */
export async function stage(source: string, output: string) {
  await stageProject(source, output);
  return output;
}

/** Replace a copied schema's initial value while retaining its descriptor and identity. */
export async function overrideSchema(source: string, initial: string, statements = "") {
  if (!(await exists(join(source, "authored-schema.ts")))) await rename(join(source, "schema.ts"), join(source, "authored-schema.ts"));
  await writeFile(join(source, "schema.ts"), `import authored from "./authored-schema";
import {defineDocument} from "@hitslop/tinys";
${statements}
export default defineDocument(authored.descriptor.properties, {initial:${initial}});
`);
}
