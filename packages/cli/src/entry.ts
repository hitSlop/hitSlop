import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { exists } from "./fs";

/** Exact-case conventional components; an authored main.ts owns all registration. */
export async function discoverEntry(source: string): Promise<{ code: string; files: string[] }> {
  const names = new Set(await readdir(source));
  const has = async (name: string) => names.has(name) && (await exists(join(source, name), true));
  if (await has("main.ts"))
    return { code: 'export { default } from "./main.ts";', files: ["main.ts"] };
  if (!(await has("App.svelte"))) throw new Error(`${source} needs App.svelte (or main.ts)`);
  const files = ["App.svelte"];
  const imports = [
    'import App from "./App.svelte";',
    'import { defineSlop } from "@hitslop/document/svelte";',
  ];
  const options: string[] = [];
  if (await has("styles.css")) {
    files.push("styles.css");
    imports.push('import "./styles.css";');
  }
  for (const [file, name, key] of [
    ["Export.svelte", "Export", "export"],
    ["Icon.svelte", "Icon", "icon"],
  ]) {
    if (await has(file!)) {
      files.push(file!);
      imports.push(`import ${name} from "./${file}";`);
      options.push(`${key}: ${name}`);
    }
  }
  return {
    code: [...imports, `export default defineSlop(App, { ${options.join(", ")} });`].join("\n"),
    files,
  };
}
