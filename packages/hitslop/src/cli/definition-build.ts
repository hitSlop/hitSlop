import { build, type InlineConfig, type Plugin } from "vite";
import { createHash } from "node:crypto";
import { mkdir, readdir, realpath, writeFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve } from "node:path";
import { findEngine } from "./engine";
import { exec } from "./process";
import { refuseHostImport, sveltePlugin, uiEntrySource, uiTransform } from "./app-vite";
import { portableAssetURLs } from "./asset-url-transform";
import type { BuildInput } from "../wire/app.generated";
import { PackageFormat, RuntimeABI } from "../schema/constants";
import { MediaTypes } from "../wire/media.generated";

const virtualEntry = "\0hitslop-definition-entry";
const typeOf = (name: string) => MediaTypes[extname(name).slice(1).toLowerCase()] ?? MediaTypes.bin!;
const bytesOf = (source: string | Uint8Array) => typeof source === "string" ? new TextEncoder().encode(source) : source;
export function resourceKey(source: string | Uint8Array, name: string) {
  return `media/${createHash("sha256").update(bytesOf(source)).digest("hex")}.${typeOf(name)[1]}`;
}
type Emitted = { key: string; bytes: Uint8Array; mediaType: string };
export type DefinitionBuildOptions = { alias?: Record<string, string> };

/** Two modes of the same resolver; Vite's emitted bundle is the resource inventory.
 * The definition runs in the restricted runner before Rust accepts the inventory. */
export async function buildDefinition(source: string, stage: string, options: DefinitionBuildOptions = {}) {
  source = await realpath(resolve(source)); stage = resolve(stage);
  let sdk = "";
  const publicFiles = await readdir(join(source,"public")).catch((error: NodeJS.ErrnoException) => {
    if (error.code === "ENOENT") return [];
    throw error;
  });
  if (publicFiles.length) throw new Error("The public/ directory is not copied. Import assets explicitly from your components or declaration.");
  const entry = join(source, "slop.ts");
  const inputs = { ui: new Set<string>(), definition: new Set<string>() };
  const outputs = { ui: new Map<string, Emitted>(), definition: new Map<string, Emitted>() };
  const referencedUIAssets = new Set<string>();
  const previous = process.env.NODE_ENV;
  process.env.NODE_ENV = "production";
  try {
    for (const mode of ["definition", "ui"] as const) {
      const headless = mode === "definition";
      const inventory = outputs[mode];
      const boundary: Plugin = {
        name: "hitslop-explicit-entry", enforce: "pre",
        async resolveId(id, importer) {
          if (id === virtualEntry) {
            const resolved = await this.resolve("hitslop", entry, { skipSelf: true });
            if (!resolved) this.error("Cannot resolve the project's hitslop SDK");
            sdk = dirname(resolved.id);
            return id;
          }
          refuseHostImport(id, (message) => this.error(message));
          if (headless && (/^svelte(?:\/|$)/.test(id) || id === "hitslop/svelte"))
            this.error(`Headless declaration cannot import ${id} (from ${importer})`);
          if (!importer || importer === virtualEntry) return;
          const found = await this.resolve(id, importer, { skipSelf: true });
          if (found?.id.split("?", 1)[0] === entry)
            this.error(`Only the generated entry may import the app declaration (from ${importer})`);
          if (headless && found && /\.(?:css|scss|sass|less|styl)$/.test(found.id.split("?",1)[0]!))
            return {id:`\0hitslop-empty-style:${found.id}.js`,moduleSideEffects:false};
          return found;
        },
        load(id) {
          const clean = id.split("?", 1)[0]!;
          if (id.startsWith("\0hitslop-empty-style:")) return "export default {};";
          if (id === virtualEntry) return headless
            ? `import app from ${JSON.stringify(entry)};
import {describeApp} from ${JSON.stringify(join(sdk,"app-definition.ts"))};
globalThis.__slopCommands = app.commands ?? {};
globalThis.__hitslopDescribe = () => JSON.stringify({ok:true,declaration:describeApp(app)});`
            : uiEntrySource(entry);
          if (headless && clean.endsWith(".svelte"))
            return `export default Object.freeze(${JSON.stringify({"~hitslop":"component",id:relative(source,clean)})});`;
        },
        transform(code, id) { if (!headless) return uiTransform(code, id, source, entry); },
        generateBundle() {
          for (const id of this.getModuleIds()) {
            if (!id.startsWith("\0")) inputs[mode].add(id.split("?", 1)[0]!);
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
    const program = outputs.definition.get("commands.js");
    if (!program) throw new Error("Definition build emitted no command program");
    const evaluated = await exec([await findEngine(), "--evaluate-command"], {
      cwd:"/", env:{}, timeout:4000,
      stdin:JSON.stringify({runtimeABI:RuntimeABI,mode:"definition",bundle:new TextDecoder().decode(program.bytes),request:"{}"}),
    });
    if (evaluated.code) throw new Error(`Definition evaluator failed: ${evaluated.stderr}`);
    const reply = JSON.parse(evaluated.stdout) as
      | { ok: true; declaration: ReturnType<typeof import("../sdk/app-definition").describeApp> }
      | { ok: false; error: string };
    if (reply.ok !== true) throw new Error(`Definition initialization failed: ${reply.error}`);
    const {artwork = {}, ...declaration} = reply.declaration;
    const hasCommands = declaration.commands.length > 0;
    const emittedKey = (url: unknown, role: string): string => {
      if (typeof url !== "string" || !url.startsWith("/assets/")) throw new Error(`${role} must reference an imported asset`);
      const key = url.slice("/assets/".length);
      if (!outputs.definition.has(key)) throw new Error(`${role} references an asset not emitted by the definition build: ${key}`);
      return key;
    };
    const skin = declaration.window.kind === "skin" ? emittedKey(declaration.window.image,"window.image") : undefined;
    const artworkKeys = Object.fromEntries(Object.entries(artwork).map(([role,url]) => [role,emittedKey(url,`artwork.${role}`)]));
    // Vite may emit an imported asset even after its only JS use was tree-shaken.
    // Use Vite's rendered dependency metadata, never a text search for URLs.
    for (const key of Object.values(artworkKeys))
      if (!referencedUIAssets.has(key)) outputs.ui.delete(key);
    const all = new Map(outputs.ui);
    for (const [key, value] of outputs.definition) {
      const other = all.get(key);
      if (other && !Buffer.from(other.bytes).equals(value.bytes)) throw new Error(`Conflicting emitted resource: ${key}`);
      all.set(key,value);
    }
    for (const {key,bytes} of all.values()) {
      if (!/\.(?:js|css)$/.test(key)) continue;
      const text = new TextDecoder().decode(bytes);
      if (text.includes(source) || text.includes(sdk)) throw new Error(`Local source path in ${key}`);
      for (const match of text.matchAll(/\/assets\/([a-zA-Z0-9_./-]+)/g)) {
        if (!all.has(match[1]!)) throw new Error(`${key} references an asset no build emitted: ${match[1]}`);
      }
      if (key === "ui.js" && /messageHandlers|__slop\b/.test(text)) throw new Error("App code cannot access the host bridge");
      if ((key.endsWith(".css") && /@import\s+(?:url\()?\s*["']?https?:|@font-face\s*\{[^}]*url\(\s*["']?https?:/i.test(text)) || /\bimport\s*\(\s*["'`]https?:/.test(text))
        throw new Error("Apps cannot depend on remote resources to boot");
    }
    const resources: BuildInput["resources"] = [];
    const stagedArtwork: BuildInput["artwork"] = {};
    for (const [role,key] of Object.entries(artworkKeys)) {
      if (role !== "preview" && role !== "icon") throw new Error(`Unknown artwork role: ${role}`);
      const path = join("artwork", `${role}.png`);
      await mkdir(join(stage,"artwork"),{recursive:true});
      await writeFile(join(stage,path),all.get(key)!.bytes);
      stagedArtwork[role] = path;
    }
    for (const value of all.values()) {
      if (value.key === "commands.js" && !hasCommands) continue;
      if (Object.values(artworkKeys).includes(value.key) && value.key !== skin && !outputs.ui.has(value.key)) continue;
      const path = join("resources",value.key);
      await mkdir(dirname(join(stage,path)),{recursive:true});
      await writeFile(join(stage,path),value.bytes);
      resources.push({kind:value.key === "commands.js" ? "command" : "app",key:value.key,mediaType:value.mediaType,path});
    }
    const input: BuildInput = {packageFormat:PackageFormat,runtimeABI:RuntimeABI,declaration,roles:{ui:"ui.js",...(hasCommands ? {commands:"commands.js"} : {}),...(outputs.ui.has("ui.css") ? {style:"ui.css"} : {}),...(skin ? {skin} : {})},resources,artwork:stagedArtwork};
    return {input,dependencies:{ui:[...inputs.ui],definition:[...inputs.definition]},uiBytes:outputs.ui.get("ui.js")!.bytes.length};
  } finally {
    if (previous === undefined) delete process.env.NODE_ENV;
    else process.env.NODE_ENV = previous;
  }
}
