/** The Shape Lab launcher, for developers: list the window variants, build one, or open an
 * editable copy in the app. The variants are presentation fixtures (lib/native-fixtures.ts). */
import { repository } from "../lib/artifacts";
import { mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { run } from "../../packages/cli/src/process";
import { createDocument } from "../lib/native";
import { buildPresentationFixtures, buildShapeLabVariant, shapeLabVariants, type ShapeLabVariant } from "../lib/native-fixtures";

if (import.meta.main) {
  const [command = "list", name = "rounded", mode] = process.argv.slice(2);
  if (command === "list") {
    for (const [key, value] of Object.entries(shapeLabVariants))
      console.log(`${key.padEnd(9)} ready — ${value.name}`);
  } else if (command === "fixtures") {
    console.log(JSON.stringify(await buildPresentationFixtures(), null, 2));
  } else {
    if (
      !["build", "open"].includes(command) ||
      !Object.hasOwn(shapeLabVariants, name) ||
      (mode !== undefined && mode !== "--fallback")
    )
      throw new Error(
        "Usage: bun run shape:lab [list | fixtures | build VARIANT | open VARIANT] [--fallback]",
      );
    const master = await buildShapeLabVariant(name as ShapeLabVariant, mode === "--fallback");
    console.log(master);
    if (command === "open") {
      const app = resolve(
        process.env.HITSLOP_APP ?? join(repository, "generated/app/hitSlop.app"),
      );
      if (!existsSync(app))
        throw new Error("Build the matching app with bun run apple:build, or set HITSLOP_APP.");
      const copy = join(
        repository,
        ".hitslop/shape-lab/documents",
        `${name}-${crypto.randomUUID()}.slop`,
      );
      await mkdir(dirname(copy), { recursive: true });
      // A copy of a template is a template; the app's engine creates a document from it.
      await createDocument(master, copy, { engine: join(app, "Contents/Helpers/slop-engine") });
      await run(["/usr/bin/open", "-a", app, copy], { failure: "Could not open Shape Lab" });
      console.log(`Editable copy: ${copy}`);
    }
  }
}
