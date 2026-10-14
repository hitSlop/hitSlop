import { build, type InlineConfig, type Plugin } from "vite";
import { readdir } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import {
  discoverRoles,
  refuseRoleImport,
  refusePageRole,
  sveltePlugin,
  uiEntrySource,
} from "./app-vite";
import { portableAssetURLs } from "./asset-url-transform";
import {
  resourceKey,
  bytesOf,
  typeOf,
  addressResources,
  type Emitted,
} from "./definition-resources";
import { buildCapture, refuseHostRuntime } from "./capture-build";
export type DefinitionBuildOptions = { alias?: Record<string, string> };
const virtualEntry = "\0hitslop-definition-entry";

/** Build disconnected role graphs and collect Vite's complete resource inventory. */
export async function buildGraphs(source: string, options: DefinitionBuildOptions) {
  let sdk = "";
  const publicFiles = await readdir(join(source, "public")).catch(
    (error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") return [];
      throw error;
    },
  );
  if (publicFiles.length)
    throw new Error(
      "The public/ directory is not copied. Import assets explicitly from your components or declaration.",
    );
  const roles = await discoverRoles(source);
  const entry = roles.metadata;
  const inputs = {
    ui: new Set<string>(),
    definition: new Set<string>(),
    capture: new Set<string>(),
  };
  const outputs = { ui: new Map<string, Emitted>(), definition: new Map<string, Emitted>() };
  const referencedUIAssets = new Set<string>();
  const previous = process.env.NODE_ENV;
  process.env.NODE_ENV = "production";
  try {
    for (const mode of ["definition", "ui"] as const) {
      const headless = mode === "definition";
      const boundary: Plugin = {
        name: "hitslop-explicit-entry",
        enforce: "pre",
        async resolveId(id, importer, resolveOptions) {
          if (id === virtualEntry) {
            const resolved = await this.resolve("hitslop", entry, { skipSelf: true });
            if (!resolved) return this.error("Cannot resolve the project's hitslop SDK");
            sdk = dirname(resolved.id);
            return id;
          }
          refuseRoleImport(id, importer, roles, headless, (message) => this.error(message));
          if (!importer || importer === virtualEntry) return;
          // Preserve require/import conditions when checking the resolved role boundary.
          const found = await this.resolve(id, importer, { ...resolveOptions, skipSelf: true });
          if (found) {
            if (headless)
              refuseRoleImport(found.id, importer, roles, true, (message) => this.error(message));
            else refusePageRole(found.id, roles, (message) => this.error(message));
          }
          return found;
        },
        load(id) {
          if (id !== virtualEntry) return;
          return headless
            ? `import app from ${JSON.stringify(roles.metadata)};
import document from ${JSON.stringify(roles.schema)};
${roles.commands ? `import commands from ${JSON.stringify(roles.commands)}; if (commands === undefined) throw new Error("commands.ts must default-export defineCommands(document, specs)");` : "const commands = undefined;"}
import {describeApp} from ${JSON.stringify(join(sdk, "app-definition.ts"))};
globalThis.__slopCommands = commands;
globalThis.__slopDescribe = () => JSON.stringify({ok:true,declaration:describeApp(app,document,commands,${JSON.stringify({ export: !!roles.export, icon: !!roles.icon, preview: !!roles.preview, ...(roles.widget ? { widget: {} } : {}) })})});`
            : uiEntrySource(roles);
        },
        generateBundle() {
          const clean = (id: string) => id.split("?", 1)[0]!;
          for (const id of this.getModuleIds()) {
            if (!id.startsWith("\0")) inputs[mode].add(clean(id));
            refuseHostRuntime(id, sdk, (message) => this.error(message));
          }
        },
      };
      const config: InlineConfig = {
        root: source,
        configFile: false,
        envFile: false,
        publicDir: false,
        logLevel: "error",
        base: "/assets/",
        resolve: { alias: options.alias, dedupe: ["svelte"], tsconfigPaths: true },
        plugins: [
          boundary,
          ...(headless
            ? [
                {
                  name: "hitslop-portable-asset-urls",
                  enforce: "post" as const,
                  renderChunk(code: string, chunk: { fileName: string }) {
                    if (!code.includes("/assets/")) return;
                    return portableAssetURLs(code, chunk.fileName);
                  },
                },
              ]
            : [sveltePlugin()]),
        ],
        build: {
          write: false,
          target: "safari17",
          minify: "oxc",
          assetsInlineLimit: 0,
          cssCodeSplit: false,
          modulePreload: false,
          rolldownOptions: {
            input: virtualEntry,
            preserveEntrySignatures: "strict",
            output: {
              format: headless ? "iife" : "es",
              codeSplitting: false,
              entryFileNames: headless ? "commands.js" : "ui.js",
              assetFileNames(asset) {
                // Vite gives the combined stylesheet a synthetic source name. CSS URL
                // imports instead carry their real source path, even for style.css?url.
                if (
                  !headless &&
                  typeof asset.source === "string" &&
                  asset.names.includes("style.css") &&
                  asset.originalFileNames.includes("style.css")
                )
                  return "ui.css";
                return resourceKey(asset.source, asset.names[0] ?? "asset.bin");
              },
            },
          },
        },
      };
      // CSS and asset plugins finish emitting after generateBundle on pre plugins.
      // The returned output is the complete emit, including those late assets.
      const result = await build(config);
      if ("on" in result) throw new Error("Definition builds cannot run in watch mode");
      for (const output of Array.isArray(result) ? result : [result]) {
        for (const item of output.output) {
          const bytes = bytesOf(item.type === "chunk" ? item.code : item.source);
          outputs[mode].set(item.fileName, {
            key: item.fileName,
            bytes,
            mediaType: typeOf(item.fileName)[0]!,
          });
          if (item.type === "asset") {
            for (const name of item.originalFileNames) inputs[mode].add(resolve(source, name));
          }
          if (!headless && item.type === "chunk") {
            const metadata = (
              item as typeof item & { viteMetadata?: { importedAssets: Set<string> } }
            ).viteMetadata;
            if (!metadata) throw new Error("Vite emitted a UI chunk without asset metadata");
            for (const key of metadata.importedAssets) referencedUIAssets.add(key);
          }
        }
      }
      const addressed = addressResources(outputs[mode]);
      if (!headless) {
        const referenced = [...referencedUIAssets];
        referencedUIAssets.clear();
        for (const key of referenced) referencedUIAssets.add(addressed.get(key) ?? key);
      }
    }
    for (const role of ["widget", "icon", "preview", "export"] as const) {
      const capture = await buildCapture(role, {
        source,
        roles,
        sdk,
        options,
      });
      if (!capture) continue;
      outputs.definition.set(`${role}.js`, {
        key: `${role}.js`,
        bytes: capture.bytes,
        mediaType: "text/javascript",
      });
      for (const path of capture.dependencies) inputs.capture.add(path);
    }
    return { inputs, outputs, referencedUIAssets, sdk };
  } finally {
    if (previous === undefined) delete process.env.NODE_ENV;
    else process.env.NODE_ENV = previous;
  }
}
