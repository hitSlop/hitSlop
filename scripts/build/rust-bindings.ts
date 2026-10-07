import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { repository, writeIfChanged } from "../lib/artifacts";

/** ts-rs follows dependencies from the Rust contract roots. Generate into a fresh
 * directory so --check compares bytes without changing the working tree. */
export async function rustBindings(check: boolean) {
  const stage = await mkdtemp(join(tmpdir(), "hitslop-bindings-"));
  try {
    const child = Bun.spawn(["cargo", "run", "--quiet", "--locked", "-p", "hitslop-core", "--features", "ts",
      "--example", "export_bindings", "--", stage], { cwd: repository, stdout: "inherit", stderr: "inherit" });
    if (await child.exited) throw new Error("Rust contract export failed");
    const destination = join(repository, "packages/hitslop/src/wire");
    const generated = (await readdir(stage)).sort();
    if (generated.some(name => !name.endsWith(".generated.ts")))
      throw new Error("Rust contracts must export to a .generated.ts module");
    const current = (await readdir(destination).catch(() => [] as string[])).filter(name => name.endsWith(".generated.ts"));
    for (const name of current) {
      if (generated.includes(name)) continue;
      if (check) throw new Error(`Obsolete generated contract: ${name}`);
      await rm(join(destination, name));
    }
    for (const name of generated) {
      // ts-rs leaves a space before line breaks around field documentation.
      const value = (await readFile(join(stage, name), "utf8")).replace(/[\t ]+$/gm, "");
      const path = join(destination, name);
      if (check) {
        if (await readFile(path, "utf8").catch(() => undefined) !== value) throw new Error(`Generated drift: ${name}`);
      } else await writeIfChanged(path, value);
    }
  } finally { await rm(stage, { recursive: true, force: true }); }
}
