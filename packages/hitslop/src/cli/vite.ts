import type { InlineConfig, Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { dirname, join, resolve } from "node:path";
import { realpath } from "node:fs/promises";
import { stripCommandBodies } from "./command-transform";
export const virtualEntry = "\0hitslop-app";

/** Development uses the same explicit declaration and command stubs as the UI build.
 * Vite owns source resolution and HMR; the Rust owner supplies document semantics. */
export async function appConfig(source: string): Promise<InlineConfig> {
  source = await realpath(resolve(source));
  let sdk: string | undefined;
  const entry = join(source, "slop.ts");
  const app: Plugin = {
    name: "hitslop-app", enforce: "pre",
    async resolveId(id, importer, options) {
      if (id === "/assets/ui.js" || id === virtualEntry) return virtualEntry;
      if (/^(?:loro-crdt|@hitslop\/shell)(\/|$)|^\/__shell__\//.test(id))
        this.error(`App code cannot import ${id}; use the document SDK`);
      if (importer && importer !== virtualEntry && !(options as { scan?: boolean }).scan) {
        const found = await this.resolve(id, importer, {skipSelf:true});
        if (found?.id.split("?",1)[0] === entry) this.error("Only the generated entry imports the app declaration");
      }
    },
    load(id) {
      if (id === virtualEntry) return `import app from ${JSON.stringify(entry)};
import {svelteApp} from "hitslop/svelte";
export default svelteApp(app.view,{schema:app.document,export:app.export,icon:app.icon,commands:app.commands});`;
    },
    async transform(code, id) {
      const clean = id.split("?",1)[0]!;
      if (clean.startsWith(source + "/") && !clean.includes("/node_modules/") && /\.[cm]?[jt]s$/.test(clean)) {
        if (!sdk) {
          const resolved = await this.resolve("hitslop", entry, { skipSelf: true });
          if (!resolved) this.error("Cannot resolve the project's hitslop SDK");
          sdk = dirname(resolved.id);
        }
        return stripCommandBodies(code, clean, join(sdk,"command-stub.ts"), clean === entry);
      }
    },
  };
  return {
    root:source, base:"/__app__/", configFile:false, envFile:false, publicDir:false, logLevel:"error",
    resolve:{dedupe:["svelte"],tsconfigPaths:true},
    optimizeDeps:{entries:[entry],exclude:["hitslop"]},
    plugins:[app,svelte({configFile:false,compilerOptions:{cssHash:({hash,css}) => `svelte-${hash(css)}`}})],
  };
}
