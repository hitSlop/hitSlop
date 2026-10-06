import { build, type Plugin, type InlineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { isAbsolute, join, relative, resolve } from "node:path";
import { readFile, writeFile } from "node:fs/promises";
import { discoverEntry } from "./entry";
import { exists } from "./fs";

/** Compiles the app into `stage/assets` and returns the module IDs it bundled. */
export type AppCompiler = (source: string, stage: string) => Promise<string[]>;
/**
 * App bundles contain authored code, Svelte and the app-side SDK (src/app plus pure
 * schema/theme helpers). The engine and host lifecycle ship with hitSlop; apps reach
 * them only through the ctx passed to mount.
 */
const shellSource = /(?:packages\/hitslop\/src\/shell|hitslop\/src\/shell)(?:\/|$)/;
/** Reject engine code, host bridge access and remote resources needed to boot. */
function checkAppBundle(inputs: string[], script: string, css: string) {
  for (const input of inputs) {
    // Vite adds cache/transform queries to installed dependency module IDs.
    const module = input.split(/[?#]/, 1)[0]!;
    // An app never embeds a document engine: the host's core owns the document.
    if (/loro-crdt/.test(input) || shellSource.test(module))
      throw new Error(`Embedded runtime code rejected: ${input}`);
  }
  if (/messageHandlers|__slop\b/.test(script))
    throw new Error("App code cannot use the host bridge; use ctx from the document SDK");
  const remote =
    /@import\s+(?:url\()?\s*["']?https?:|@font-face\s*\{[^}]*url\(\s*["']?https?:/i.exec(css);
  if (remote || /\bimport\s*\(\s*["'`]https?:/.test(script))
    throw new Error(
      "Apps must not need remote stylesheets, fonts or scripts to start; copy them into assets/",
    );
}
const entry = "/assets/app.js";
/** The module ID of the generated entry; dev invalidates it when components change. */
export const virtualEntry = "\0hitslop-app";

// App compilation is independent of metadata evaluation and package publication.
export async function appConfig(source: string): Promise<InlineConfig> {
  source = resolve(source);
  // slop.ts declares the app's row for the build; the app itself never loads it.
  const metadata = join(source, "slop.ts");
  const app: Plugin = {
    name: "hitslop-app",
    enforce: "pre",
    async config() {
      const discovered = await discoverEntry(source);
      return {
        optimizeDeps: {
          entries: discovered.files.map((file) => join(source, file)),
          // Authored Svelte/TypeScript must go through the normal source pipeline,
          // including when this SDK is installed rather than workspace-linked.
          exclude: ["hitslop"],
          // The excluded SDK's validators are otherwise discovered after the app
          // mounts, forcing a reload that interrupts edits and captures.
          include: ["hitslop > typebox", "hitslop > typebox/value"],
        },
      };
    },
    resolveId(id) {
      if (id === entry) return virtualEntry;
      if (/^(?:loro-crdt|@hitslop\/shell)(\/|$)|^\/__shell__\//.test(id))
        throw new Error(`App code cannot import ${id}; use ctx from the document SDK`);
      if (id.startsWith("/assets/")) return join(source, decodeURIComponent(id.slice(1)));
    },
    async load(id) {
      if (id.split(/[?#]/, 1)[0] === metadata)
        throw new Error(
          "slop.ts is build-only; move values the app shares into their own module, as shape-lab's variant.ts does",
        );
      if (id !== virtualEntry) return;
      const discovered = await discoverEntry(source);
      return discovered.code.replace(/(["'])\.\/([^"']+)\1/g, (_, quote, path) =>
        JSON.stringify(join(source, path)),
      );
    },
  };
  const devAudit: Plugin = {
    name: "hitslop-dev-audit",
    apply: "serve",
    enforce: "post",
    transform(code, id) {
      checkAppBundle([id], id.includes(".css") ? "" : code, id.includes(".css") ? code : "");
    },
  };
  return {
    root: source,
    configFile: false,
    envFile: false,
    publicDir: false,
    logLevel: "error",
    resolve: {
      dedupe: ["svelte"],
    },
    plugins: [
      app,
      svelte({
        configFile: false,
        compilerOptions: {
          // Match the portable hash policy used by the existing compiler.
          cssHash: ({ hash, css }) => `svelte-${hash(css)}`,
        },
      }),
      devAudit,
    ],
  };
}

export const compileAppWithVite: AppCompiler = async (source, stage) => {
  // Vite and Svelte select production output from NODE_ENV. Restore it, so a dev server
  // later in the same process still compiles with HMR.
  const previous = process.env.NODE_ENV;
  process.env.NODE_ENV = "production";
  try {
    const config = await appConfig(source);
    const inputs: string[] = [];
    await build({
      ...config,
      base: "/assets/",
      plugins: [
        ...(config.plugins ?? []),
        {
          name: "hitslop-input-audit",
          generateBundle() {
            inputs.push(...this.getModuleIds());
          },
        },
      ],
      build: {
        outDir: join(stage, "assets"),
        emptyOutDir: false,
        target: "safari17",
        minify: process.env.HITSLOP_DEBUG_BUILD === "1" ? false : "oxc",
        assetsInlineLimit: 0,
        cssCodeSplit: false,
        modulePreload: false,
        rolldownOptions: {
          input: entry,
          preserveEntrySignatures: "strict",
          output: {
            format: "es",
            entryFileNames: "app.js",
            codeSplitting: false,
            assetFileNames(asset) {
              if (asset.names.some((name) => name.endsWith(".css"))) return "app.css";
              const original = asset.originalFileNames[0];
              if (original) {
                const path = relative(join(source, "assets"), resolve(source, original));
                if (!isAbsolute(path) && path !== ".." && !path.startsWith("../")) return path;
              }
              return "[name]-[hash][extname]";
            },
          },
        },
      },
    });
    const css = join(stage, "assets/app.css");
    if (!(await exists(css))) await writeFile(css, "");
    checkAppBundle(
      inputs,
      await readFile(join(stage, "assets/app.js"), "utf8"),
      await readFile(css, "utf8"),
    );
    return inputs;
  } finally {
    if (previous === undefined) delete process.env.NODE_ENV;
    else process.env.NODE_ENV = previous;
  }
};
