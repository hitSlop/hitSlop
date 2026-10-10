import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, extname, join } from "node:path";
import type { BuildInput } from "../wire/app.generated";
import { PackageFormat, RuntimeABI } from "../schema/constants";
import { MediaTypes } from "../wire/media.generated";
import type { buildGraphs } from "./definition-config";
import type { describeApp } from "../sdk/app-definition";

export const typeOf = (name: string) => MediaTypes[extname(name).slice(1).toLowerCase()] ?? MediaTypes.bin!;
export const bytesOf = (source: string | Uint8Array) => typeof source === "string" ? new TextEncoder().encode(source) : source;
export function resourceKey(source: string | Uint8Array, name: string) {
  return `media/${createHash("sha256").update(bytesOf(source)).digest("hex")}.${typeOf(name)[1]}`;
}
export type Emitted = { key: string; bytes: Uint8Array; mediaType: string };

/** Validate portable resources before writing the packer's stage. */
export async function stageResources(source: string, stage: string, graphs: Awaited<ReturnType<typeof buildGraphs>>, described: ReturnType<typeof describeApp>): Promise<BuildInput> {
    const { outputs, referencedUIAssets, sdk } = graphs;
    const { artwork = {}, ...declaration } = described;
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
    return input;
}
