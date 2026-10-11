import { build, type InlineConfig, type Plugin } from "vite";
import { readdir } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { discoverRoles, refuseRoleImport, refusePageRole, sveltePlugin, uiEntrySource } from "./app-vite";
import { portableAssetURLs } from "./asset-url-transform";
import { declaresModuleState, sharedStateIn } from "./capture-state";
import { resourceKey, bytesOf, typeOf, type Emitted } from "./definition-resources";
export type DefinitionBuildOptions = { alias?: Record<string, string> };
const virtualEntry = "\0hitslop-definition-entry";

/** Build two disconnected graphs and collect Vite's complete resource inventory. */
export async function buildGraphs(source: string, options: DefinitionBuildOptions) {
  let sdk = "";
  const publicFiles = await readdir(join(source,"public")).catch((error: NodeJS.ErrnoException) => {
    if (error.code === "ENOENT") return [];
    throw error;
  });
  if (publicFiles.length) throw new Error("The public/ directory is not copied. Import assets explicitly from your components or declaration.");
  const roles = await discoverRoles(source);
  const entry = roles.metadata;
  const inputs = { ui: new Set<string>(), definition: new Set<string>() };
  const outputs = { ui: new Map<string, Emitted>(), definition: new Map<string, Emitted>() };
  const referencedUIAssets = new Set<string>();
  // The UI's module graph, and the project's modules in it with top-level $state.
  const uiImports = new Map<string, string[]>();
  const statefulModules = new Set<string>();
  const previous = process.env.NODE_ENV;
  process.env.NODE_ENV = "production";
  try {
    for (const mode of ["definition", "ui"] as const) {
      const headless = mode === "definition";
      const inventory = outputs[mode];
      const boundary: Plugin = {
        name: "hitslop-explicit-entry", enforce: "pre",
        async resolveId(id, importer, resolveOptions) {
          if (id === virtualEntry) {
            const resolved = await this.resolve("hitslop", entry, { skipSelf: true });
            if (!resolved) return this.error("Cannot resolve the project's hitslop SDK");
            sdk = dirname(resolved.id);
            return id;
          }
          refuseRoleImport(id, importer, roles, headless, message => this.error(message));
          if (!importer || importer === virtualEntry) return;
          // Preserve require/import conditions when checking the resolved role boundary.
          const found = await this.resolve(id, importer, { ...resolveOptions, skipSelf: true });
          if (found) {
            if (headless) refuseRoleImport(found.id, importer, roles, true, message => this.error(message));
            else refusePageRole(found.id, roles, message => this.error(message));
          }
          return found;
        },
        load(id) {
          if (id === virtualEntry) return headless
            ? `import app from ${JSON.stringify(roles.metadata)};
import document from ${JSON.stringify(roles.schema)};
${roles.commands ? `import commands from ${JSON.stringify(roles.commands)}; if (commands === undefined) throw new Error("commands.ts must default-export defineCommands(document, specs)");` : "const commands = undefined;"}
import {describeApp} from ${JSON.stringify(join(sdk,"app-definition.ts"))};
globalThis.__slopCommands = commands;
globalThis.__slopDescribe = () => JSON.stringify({ok:true,declaration:describeApp(app,document,commands,${JSON.stringify({export:!!roles.export,icon:!!roles.icon})})});`
            : uiEntrySource(roles);
        },
        transform(code, id) {
          const clean = id.split("?", 1)[0]!;
          if (!headless && clean.startsWith(source + "/") && !clean.includes("/node_modules/") && declaresModuleState(code, clean))
            statefulModules.add(clean);
        },
        generateBundle() {
          const clean = (id: string) => id.split("?", 1)[0]!;
          for (const id of this.getModuleIds()) {
            if (!id.startsWith("\0")) inputs[mode].add(clean(id));
            if (!headless) uiImports.set(clean(id), [...this.getModuleInfo(id)?.importedIds ?? []].map(clean));
            // The host's own runtime, not an author folder that happens to be named shell.
            if (id.includes("/node_modules/loro-crdt/") || id.startsWith(join(dirname(sdk), "shell") + "/"))
              this.error(`Embedded host runtime rejected: ${id}`);
          }
        },
      };
      const config: InlineConfig = {
        root: source, configFile:false, envFile:false, publicDir:false, logLevel:"error", base:"/assets/",
        resolve: { alias:options.alias, dedupe:["svelte"], tsconfigPaths:true },
        plugins:[boundary, ...(headless ? [{
          name:"hitslop-portable-asset-urls", enforce:"post" as const,
          renderChunk(code: string, chunk: {fileName:string}) {
            if (code.includes("/assets/")) return portableAssetURLs(code,chunk.fileName);
          },
        }] : [sveltePlugin()])],
        build: {
          write:false, target:"safari17", minify:"oxc", assetsInlineLimit:0, cssCodeSplit:false, modulePreload:false,
          rolldownOptions:{ input:virtualEntry, preserveEntrySignatures:"strict", output:{
            format:headless ? "iife" : "es", codeSplitting:false,
            entryFileNames:headless ? "commands.js" : "ui.js",
            assetFileNames(asset) {
              if (asset.names.some(name => name.endsWith(".css"))) return "ui.css";
              return resourceKey(asset.source, asset.names[0] ?? "asset.bin");
            },
          } },
        },
      };
      // CSS and asset plugins finish emitting after generateBundle on pre plugins.
      // The returned output is the complete emit, including those late assets.
      const result = await build(config);
      if ("on" in result) throw new Error("Definition builds cannot run in watch mode");
      for (const output of Array.isArray(result) ? result : [result]) {
        for (const item of output.output) {
          const bytes = bytesOf(item.type === "chunk" ? item.code : item.source);
          inventory.set(item.fileName, {key:item.fileName, bytes, mediaType:typeOf(item.fileName)[0]!});
          if (item.type === "asset") {
            for (const name of item.originalFileNames) inputs[mode].add(resolve(source,name));
          }
          if (!headless && item.type === "chunk") {
            const metadata = (item as typeof item & {viteMetadata?: {importedAssets:Set<string>}}).viteMetadata;
            if (!metadata) throw new Error("Vite emitted a UI chunk without asset metadata");
            for (const key of metadata.importedAssets) referencedUIAssets.add(key);
          }
        }
      }
    }
    // Captures render saved state in a fresh page, so a module the editor sets shows only
    // its initial value there, even when the editor is reused as the export component.
    for (const role of ["export", "icon"] as const) {
      const component = roles[role];
      if (!component) continue;
      const shared = sharedStateIn(component, uiImports, statefulModules);
      if (shared) throw new Error(`${component} (the ${role} view) imports ${relative(source, shared)}, whose $state is always its initial value in a capture. Captures render saved state in a fresh page: read doc.current there, and give a component shared with the editor that value as a prop.`);
    }
    return { inputs, outputs, referencedUIAssets, sdk };
  } finally {
    if (previous === undefined) delete process.env.NODE_ENV;
    else process.env.NODE_ENV = previous;
  }
}
