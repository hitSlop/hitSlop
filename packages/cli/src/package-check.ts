import { lstat, readdir, readFile, realpath } from "node:fs/promises";
import { join, relative } from "node:path";
import { PackageLimits } from "@hitslop/schema";

/** The package rules native open applies (`SlopPackage.swift`), checked on a built
 * template before it is published. Native open stays the authority; this lets a build on
 * any platform refuse a package the app would refuse. */
const topLevel = new Set(["manifest.json", "assets", "QuickLook", ".agents", "state.schema.json", "initial.json"]);
const forbidden = new Set(["package.json", "bun.lock", "bun.lockb", "node_modules", "source", "src", "build", ".build", ".hitslop"]);
const quickLook = new Set(["Preview.png", "Icon.png"]);

const invalid = (message: string) => new Error(`Invalid hitSlop package: ${message}`);

export async function checkTemplatePackage(root: string) {
  root = await realpath(root);
  for (const name of await readdir(root)) {
    if (!topLevel.has(name)) throw invalid(`unexpected package entry ${name}`);
  }
  let entries = 0;
  let bytes = 0;
  for (const name of await readdir(root, { recursive: true })) {
    const path = join(root, name);
    const info = await lstat(path);
    if (info.isSymbolicLink() || !(info.isFile() || info.isDirectory()))
      throw invalid("packages require regular files and directories, without symlinks");
    if (forbidden.has(name.split("/").at(-1)!.toLowerCase())) throw invalid(`packages cannot contain ${name.split("/").at(-1)}`);
    entries += 1;
    if (info.isFile()) {
      if (info.size > PackageLimits.file) throw invalid("package file exceeds 25 MiB");
      bytes += info.size;
    }
    if (entries > PackageLimits.entries || bytes > PackageLimits.bytes)
      throw invalid("immutable package exceeds 256 entries or 50 MiB");
  }
  const app = await readFile(join(root, "assets/app.js")).catch(() => {
    throw new Error("Missing assets/app.js");
  });
  if (!utf8(app)) throw invalid("assets/app.js must be UTF-8");
  for (const name of await readdir(join(root, "QuickLook")).catch(() => [] as string[])) {
    if (!quickLook.has(name)) throw invalid(`unexpected QuickLook entry ${name}`);
    png(await readFile(join(root, "QuickLook", name)), `QuickLook/${name}`);
  }
  const { presentation } = JSON.parse(await readFile(join(root, "manifest.json"), "utf8"));
  if (presentation.skin) await checkSkin(root, presentation.skin, presentation.width, presentation.height);
}

async function checkSkin(root: string, path: string, width: number, height: number) {
  const parts = path.split("/");
  if (!path || path.length > 240 || path.startsWith("/") || path.includes("\\") || path.includes("\0") ||
    parts.some((part) => !part || part === "." || part === ".."))
    throw invalid(`unsafe path ${path}`);
  const url = join(root, path);
  if (relative(root, url).startsWith("..")) throw invalid(`unsafe path ${path}`);
  const info = await lstat(url).catch(() => undefined);
  if (!info?.isFile()) throw invalid("window skin must be a regular file");
  const image = png(await readFile(url), "window skin");
  if (image.width !== width || image.height !== height)
    throw invalid(`window skin must be exactly ${width}x${height} pixels`);
  // RGBA. The app also accepts other encodings that decode with alpha; the build asks for
  // the one every decoder agrees on.
  if (image.colorType !== 6) throw invalid("window skin must be an RGBA PNG with alpha");
}

/** A PNG's header: its size within the shared image limits, and its colour type. */
export function png(bytes: Uint8Array, label: string) {
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const isPNG = bytes.length >= 33 && signature.every((byte, index) => bytes[index] === byte) &&
    String.fromCharCode(...bytes.subarray(12, 16)) === "IHDR";
  if (!isPNG) throw invalid(`${label} must be a valid PNG`);
  const width = view.getUint32(16);
  const height = view.getUint32(20);
  if (!width || !height || width > PackageLimits.imageSide || height > PackageLimits.imageSide ||
    width * height > PackageLimits.imagePixels)
    throw invalid(`${label} exceeds the PNG dimension limit`);
  return { width, height, colorType: bytes[25] };
}

function utf8(bytes: Uint8Array) {
  try {
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return true;
  } catch {
    return false;
  }
}
