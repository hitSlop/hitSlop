import type { InlineConfig, Plugin } from "vite";
import { resolve } from "node:path";
import { realpath } from "node:fs/promises";
import {
  discoverRoles,
  refuseRoleImport,
  refusePageRole,
  sveltePlugin,
  uiEntrySource,
} from "./app-vite";
export const virtualEntry = "\0hitslop-app";

/** Development and packing use the same role discovery and page boundary. */
export async function appConfig(source: string): Promise<InlineConfig> {
  source = await realpath(resolve(source));
  const roles = await discoverRoles(source);
  const app: Plugin = {
    name: "hitslop-app",
    enforce: "pre",
    async resolveId(id, importer, resolveOptions) {
      if (id === "/assets/ui.js" || id === virtualEntry) return virtualEntry;
      refuseRoleImport(id, importer, roles, false, (message) => this.error(message));
      if (importer && importer !== virtualEntry) {
        const found = await this.resolve(id, importer, { ...resolveOptions, skipSelf: true });
        if (found) refusePageRole(found.id, roles, (message) => this.error(message));
        return found;
      }
    },
    async load(id) {
      if (id === virtualEntry) return uiEntrySource(await discoverRoles(source));
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
      entries: [
        roles.view,
        ...[roles.export, roles.icon, roles.widget].filter((path): path is string => !!path),
      ],
      exclude: ["hitslop"],
    },
    plugins: [app, sveltePlugin()],
  };
}
