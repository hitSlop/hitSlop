import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readdir } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

const roleNames = ["slop.ts", "schema.ts", "App.svelte", "Export.svelte", "Icon.svelte", "commands.ts"] as const;
export type AppRoles = { metadata: string; schema: string; view: string; export?: string; icon?: string; commands?: string };
export function roleName(file: string): string | undefined {
  const name = basename(file).toLowerCase();
  return roleNames.find(role => role.toLowerCase() === name) ?? (name === "command.ts" ? "commands.ts" : undefined);
}
/** Fixed root roles keep definition and page dependencies separate without source transforms. */
export async function discoverRoles(source: string): Promise<AppRoles> {
  const files = new Set(await readdir(source));
  for (const file of files) {
    const role = roleName(file);
    if (role && file !== role) throw new Error(`${file}: did you mean ${role}? Role filenames are case-sensitive.`);
  }
  for (const required of ["slop.ts", "schema.ts", "App.svelte"]) {
    if (!files.has(required)) throw new Error(`Missing required ${required}`);
  }
  return {
    metadata: join(source, "slop.ts"), schema: join(source, "schema.ts"), view: join(source, "App.svelte"),
    ...(files.has("Export.svelte") ? { export: join(source, "Export.svelte") } : {}),
    ...(files.has("Icon.svelte") ? { icon: join(source, "Icon.svelte") } : {}),
    ...(files.has("commands.ts") ? { commands: join(source, "commands.ts") } : {}),
  };
}
/** Host runtime an app may not import: the page shell and the document engine. */
export function refuseHostImport(id: string, error: (message: string) => never) {
  if (/^(?:loro-crdt|@hitslop\/shell)(\/|$)|^\/__shell__\//.test(id)) error(`App code cannot import ${id}; use the document SDK`);
}
export function refuseRoleImport(id: string, importer: string | undefined, roles: AppRoles, headless: boolean, error: (message: string) => never) {
  refuseHostImport(id, error);
  if (id === "hitslop/commands" && (!headless || importer?.split("?", 1)[0] !== roles.commands))
    error(`Only root commands.ts may import hitslop/commands (from ${importer})`);
  if (headless && (/^svelte(?:\/|$)/.test(id) || id === "hitslop/svelte" || /\.(?:svelte|css|scss|sass|less|styl)(?:\?|$)/.test(id)))
    error(`Definition modules cannot import ${id} (from ${importer}); import it from App.svelte, Export.svelte or Icon.svelte`);
}
export function refusePageRole(id: string, roles: AppRoles, error: (message: string) => never) {
  const clean = id.split("?", 1)[0];
  if (clean === roles.metadata) error("slop.ts is metadata; the page cannot import it");
  if (clean === roles.commands || clean === join(dirname(roles.metadata), "commands.ts")) error("commands.ts is agent-only; the page cannot import it");
}
export const uiEntrySource = (roles: AppRoles) => `import view from ${JSON.stringify(roles.view)};
import document from ${JSON.stringify(roles.schema)};
${roles.export ? `import exportView from ${JSON.stringify(roles.export)};` : "const exportView = undefined;"}
${roles.icon ? `import icon from ${JSON.stringify(roles.icon)};` : "const icon = undefined;"}
import {svelteApp} from "hitslop/svelte";
export default svelteApp(view,{schema:document,export:exportView,icon});`;
export const sveltePlugin = () => svelte({ configFile: false, compilerOptions: { cssHash: ({ hash, css }) => `svelte-${hash(css)}` } });
