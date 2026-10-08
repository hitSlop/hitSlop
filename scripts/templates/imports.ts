import { readFile } from "node:fs/promises";
import { dirname } from "node:path";

/** Files reachable from `entries` through relative imports, as absolute paths. Package
 * imports are not followed, and only script files are scanned for further imports. */
export async function localImports(entries: string[]): Promise<string[]> {
  const transpiler = new Bun.Transpiler({ loader: "ts" });
  const found = new Set<string>();
  const pending = [...entries];
  while (pending.length) {
    const path = pending.pop()!;
    if (found.has(path)) continue;
    found.add(path);
    if (!/\.[cm]?[jt]sx?$/.test(path)) continue;
    for (const { path: specifier } of transpiler.scanImports(await readFile(path, "utf8")))
      if (specifier.startsWith(".")) pending.push(Bun.resolveSync(specifier, dirname(path)));
  }
  return [...found];
}
