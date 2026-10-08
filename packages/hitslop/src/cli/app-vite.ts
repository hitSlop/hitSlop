import { svelte } from "@sveltejs/vite-plugin-svelte";
import { stripCommandBodies } from "./command-transform";

// What the dev server and both builds share, so the preview compiles the app the way the
// package does.
/** Host runtime an app may not import: the page shell and the document engine. */
export function refuseHostImport(id: string, error: (message: string) => never) {
  if (/^(?:loro-crdt|@hitslop\/shell)(\/|$)|^\/__shell__\//.test(id)) error(`App code cannot import ${id}; use the document SDK`);
}
/** The UI entry: the declaration's roles, mounted through the Svelte adapter. */
export const uiEntrySource = (entry: string) => `import app from ${JSON.stringify(entry)};
import {svelteApp} from "hitslop/svelte";
export default svelteApp(app.view,{schema:app.document,export:app.export,icon:app.icon,commands:app.commands});`;
export const sveltePlugin = () => svelte({ configFile: false, compilerOptions: { cssHash: ({ hash, css }) => `svelte-${hash(css)}` } });
/** The project's own script modules lose command bodies in the UI; dependencies are not ours. */
export function uiTransform(code: string, id: string, source: string, entry: string) {
  const clean = id.split("?", 1)[0]!;
  if (clean.startsWith(source + "/") && !clean.includes("/node_modules/") && /\.[cm]?[jt]sx?$/.test(clean))
    return stripCommandBodies(code, clean, clean === entry);
}
