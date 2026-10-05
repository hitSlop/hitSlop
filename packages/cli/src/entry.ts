import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { exists } from "./fs";

/** The files whose presence decides the entry: adding or removing one changes it. */
export const entryFiles = ["main.ts", "App.svelte", "schema.ts", "styles.css", "Export.svelte", "Icon.svelte"];

/** Exact-case conventional components; every app uses the generated Svelte entry. */
export async function discoverEntry(source: string): Promise<{ code: string; files: string[] }> {
  const names = new Set(await readdir(source));
  const has = async (name: string) => names.has(name) && (await exists(join(source, name), true));
  if (await has("main.ts")) throw new Error("main.ts is not supported; use App.svelte, with optional Export.svelte and Icon.svelte");
  if (!(await has("App.svelte"))) throw new Error(`${source} needs App.svelte`);
  const files = ["App.svelte", "schema.ts"];
  // schema.ts is required: its default export becomes the live document once mounted.
  const imports = [
    'import App from "./App.svelte";',
    'import schema from "./schema.ts";',
    'import { svelteApp } from "@hitslop/document/svelte";',
  ];
  const options = ["schema"];
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
    code: [...imports, `export default svelteApp(App, { ${options.join(", ")} });`].join("\n"),
    files,
  };
}
