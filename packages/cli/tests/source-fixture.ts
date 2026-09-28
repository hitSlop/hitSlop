import { cp, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

/** Keep a copied project's relative tsconfig inheritance valid at its new location. */
export async function copySourceFixture(source: string, destination: string) {
  await cp(source, destination, { recursive: true });
  const path = join(destination, "tsconfig.json");
  const config = JSON.parse(await readFile(path, "utf8"));
  if (typeof config.extends === "string" && config.extends.startsWith(".")) {
    config.extends = resolve(source, config.extends);
    await writeFile(path, JSON.stringify(config, null, 2));
  }
}
