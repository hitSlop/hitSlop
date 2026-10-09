import { CommandBindings } from "./command-bindings";
import type { InlineConfig, Plugin } from "vite";
import { join, resolve } from "node:path";
import { realpath } from "node:fs/promises";
import { refuseHostImport, sveltePlugin, uiEntrySource, uiTransform } from "./app-vite";
export const virtualEntry = "\0hitslop-app";

/** Development uses the same explicit declaration and command stubs as the UI build.
 * Vite owns source resolution and HMR; the Rust owner supplies document semantics. */
export async function appConfig(source: string): Promise<InlineConfig> {
  source = await realpath(resolve(source));
  const entry = join(source, "slop.ts");
  const bindings = new CommandBindings();
  const app: Plugin = {
    name: "hitslop-app", enforce: "pre",
    async resolveId(id, importer, options) {
      if (id === "/assets/ui.js" || id === virtualEntry) return virtualEntry;
      refuseHostImport(id, (message) => this.error(message));
      if (importer && importer !== virtualEntry && !(options as { scan?: boolean }).scan) {
        const found = await this.resolve(id, importer, {skipSelf:true});
        if (found?.id.split("?",1)[0] === entry) this.error("Only the generated entry imports the app declaration");
      }
    },
    load(id) {
      if (id === virtualEntry) return uiEntrySource(entry);
    },
    watchChange() { bindings.clear(); },
    async transform(code, id) {
      const clean = id.split("?", 1)[0]!;
      const own = clean === id && clean.startsWith(source + "/") && !clean.includes("/node_modules/") && /\.(?:[cm]?[jt]sx?|svelte)$/.test(clean);
      const members = own ? await bindings.members(code, clean, source, async (specifier, importer) =>
        (await this.resolve(specifier, importer, { skipSelf: true }))?.id) : new Set<number>();
      return uiTransform(code, id, source, entry, undefined, members);
    },
  };
  return {
    root:source, base:"/__app__/", configFile:false, envFile:false, publicDir:false, logLevel:"error",
    resolve:{dedupe:["svelte"],tsconfigPaths:true},
    optimizeDeps:{entries:[entry],exclude:["hitslop"]},
    plugins:[app,sveltePlugin()],
  };
}
