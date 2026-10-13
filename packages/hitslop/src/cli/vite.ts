import type { InlineConfig, Plugin } from "vite";
import { join, resolve } from "node:path";
import { realpath } from "node:fs/promises";
import {
  discoverRoles,
  refuseRoleImport,
  refusePageRole,
  sveltePlugin,
  uiEntrySource,
  type AppRoles,
} from "./app-vite";
import { cliRoot } from "./paths";
export const virtualEntry = "\0hitslop-app";
export const viewsEntry = "\0hitslop-views";

/** `slop dev` mounts one view at a time through the dev-only wrapper; the editor is the
 * production entry, which imports App and the schema only. */
const viewsEntrySource = (roles: AppRoles) => `import editor from "/assets/ui.js";
import preview from ${JSON.stringify(roles.preview)};
import icon from ${JSON.stringify(roles.icon)};
${roles.widget ? `import widget from ${JSON.stringify(roles.widget)};` : "const widget = undefined;"}
import {previewViews} from ${JSON.stringify(join(cliRoot, "src/cli/preview-views.svelte.ts"))};
export default previewViews(editor,{preview,icon,widget});`;

/** The development server's page boundary, with the same role discovery as packing. */
export async function appConfig(source: string): Promise<InlineConfig> {
  source = await realpath(resolve(source));
  // Rediscovered when the views entry loads again after a role is added or removed.
  let roles = await discoverRoles(source);
  const app: Plugin = {
    name: "hitslop-app",
    enforce: "pre",
    async resolveId(id, importer, resolveOptions) {
      if (id === "/assets/ui.js" || id === virtualEntry) return virtualEntry;
      if (id === "/assets/views.js" || id === viewsEntry) return viewsEntry;
      refuseRoleImport(id, importer, roles, false, (message) => this.error(message));
      if (importer && importer !== virtualEntry && importer !== viewsEntry) {
        const found = await this.resolve(id, importer, { ...resolveOptions, skipSelf: true });
        if (found) refusePageRole(found.id, roles, (message) => this.error(message));
        return found;
      }
    },
    async load(id) {
      if (id === virtualEntry) return uiEntrySource(roles);
      if (id === viewsEntry) return viewsEntrySource((roles = await discoverRoles(source)));
    },
  };
  return {
    root: source,
    base: "/__app__/",
    configFile: false,
    envFile: false,
    publicDir: false,
    logLevel: "error",
    resolve: { dedupe: ["svelte"], tsconfigPaths: true },
    optimizeDeps: {
      // The views `slop dev` serves; it has no Export view.
      entries: [roles.view, roles.preview, roles.icon, roles.widget].filter(
        (path): path is string => !!path,
      ),
      exclude: ["hitslop"],
    },
    plugins: [app, sveltePlugin()],
  };
}
