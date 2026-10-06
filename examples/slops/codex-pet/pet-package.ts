import JSZip from "jszip";

const MAX_COMPRESSED = 10 * 1024 * 1024;
const MAX_ENTRY = 25 * 1024 * 1024;
const MAX_EXPANDED = 50 * 1024 * 1024;
const MAX_ENTRIES = 256;
const MAX_MANIFEST = 64 * 1024;

export type PetMetadata = {
  id: string;
  displayName: string;
  description: string;
  spriteVersionNumber: 1 | 2;
  spritesheetPath: string;
};

export type ParsedPetPackage = PetMetadata & {
  spriteBytes: Uint8Array;
  spriteMime: "image/png" | "image/webp";
};

const u16 = (bytes: Uint8Array, at: number): number | null => at >= 0 && at + 2 <= bytes.length
  ? (bytes[at] ?? 0) | (bytes[at + 1] ?? 0) << 8
  : null;
const u32 = (bytes: Uint8Array, at: number): number | null => {
  if (at < 0 || at + 4 > bytes.length) return null;
  return ((bytes[at] ?? 0) | (bytes[at + 1] ?? 0) << 8 | (bytes[at + 2] ?? 0) << 16 | (bytes[at + 3] ?? 0) << 24) >>> 0;
};

function normalizePath(value: string): string {
  const path = value.replaceAll("\\", "/");
  const parts = path.split("/").filter(Boolean);
  if (!parts.length || path.startsWith("/") || /^[A-Za-z]:/.test(path) || parts.some((part) => part === "." || part === ".." || part.includes("\0"))) {
    throw new Error("The ZIP contains an unsafe path.");
  }
  return parts.join("/");
}

function validateArchive(bytes: Uint8Array): void {
  if (bytes.length > MAX_COMPRESSED) throw new Error("That pet ZIP is larger than 10 MB.");
  const first = Math.max(0, bytes.length - 22 - 65_535), last = bytes.length - 22;
  let end = -1;
  for (let at = last; at >= first; at -= 1) if (u32(bytes, at) === 0x06054b50) { end = at; break; }
  if (end < 0) throw new Error("That file is not a readable ZIP package.");
  const count = u16(bytes, end + 10), centralSize = u32(bytes, end + 12), centralOffset = u32(bytes, end + 16);
  if (count == null || centralSize == null || centralOffset == null || count < 1 || count > MAX_ENTRIES || centralOffset + centralSize > end) {
    throw new Error("That pet ZIP is malformed or too large.");
  }
  let cursor = centralOffset, total = 0;
  const paths = new Set<string>();
  for (let index = 0; index < count; index += 1) {
    if (u32(bytes, cursor) !== 0x02014b50) throw new Error("That pet ZIP is malformed.");
    const madeBy = u16(bytes, cursor + 4), flags = u16(bytes, cursor + 8), method = u16(bytes, cursor + 10);
    const compressed = u32(bytes, cursor + 20), size = u32(bytes, cursor + 24), external = u32(bytes, cursor + 38);
    const nameLength = u16(bytes, cursor + 28), extraLength = u16(bytes, cursor + 30), commentLength = u16(bytes, cursor + 32);
    if ([madeBy, flags, method, compressed, size, external, nameLength, extraLength, commentLength].some((value) => value == null)) {
      throw new Error("That pet ZIP is truncated.");
    }
    if (((flags as number) & 1) !== 0) throw new Error("Encrypted pet ZIPs are not supported.");
    if (![0, 8].includes(method as number)) throw new Error("That pet ZIP uses unsupported compression.");
    if (compressed === 0xffff_ffff || size === 0xffff_ffff) throw new Error("ZIP64 pet packages are not supported.");
    if ((size as number) > MAX_ENTRY) throw new Error("A file inside that pet ZIP is too large.");
    total += size as number;
    if (total > MAX_EXPANDED) throw new Error("That pet ZIP expands beyond 50 MB.");
    const platform = (madeBy as number) >>> 8;
    const unixMode = (external as number) >>> 16;
    if (platform === 3 && (unixMode & 0xf000) === 0xa000) throw new Error("Pet ZIPs cannot contain symbolic links.");
    const nameStart = cursor + 46, next = nameStart + (nameLength as number) + (extraLength as number) + (commentLength as number);
    if (next > bytes.length) throw new Error("That pet ZIP is truncated.");
    const rawPath = new TextDecoder().decode(bytes.subarray(nameStart, nameStart + (nameLength as number)));
    if (!rawPath.endsWith("/")) {
      const path = normalizePath(rawPath).toLowerCase();
      if (paths.has(path)) throw new Error("That pet ZIP contains duplicate file paths.");
      paths.add(path);
    }
    cursor = next;
  }
}

const requiredText = (value: unknown, fallback: string, limit: number): string => {
  if (value == null) return fallback;
  if (typeof value !== "string" || !value.trim() || value.length > limit) throw new Error("pet.json contains invalid text fields.");
  return value.trim();
};

export async function parsePetArchive(bytes: Uint8Array): Promise<ParsedPetPackage> {
  validateArchive(bytes);
  let zip: JSZip;
  try { zip = await JSZip.loadAsync(bytes); }
  catch { throw new Error("That pet ZIP could not be expanded."); }
  const normalized = new Map<string, Uint8Array>();
  for (const [rawPath, entry] of Object.entries(zip.files)) {
    if (entry.dir || rawPath.endsWith("/")) continue;
    normalized.set(normalizePath(rawPath), await entry.async("uint8array"));
  }
  const manifests = [...normalized.keys()].filter((path) => path.toLowerCase().endsWith("/pet.json") || path.toLowerCase() === "pet.json");
  if (manifests.length !== 1) throw new Error("A pet ZIP must contain exactly one pet.json.");
  const manifestPath = manifests[0] as string;
  const manifestBytes = normalized.get(manifestPath) as Uint8Array;
  if (manifestBytes.length > MAX_MANIFEST) throw new Error("pet.json is unexpectedly large.");
  let value: unknown;
  try { value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(manifestBytes)); }
  catch { throw new Error("pet.json is not valid UTF-8 JSON."); }
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("pet.json must contain an object.");
  const manifest = value as Record<string, unknown>;
  const version = manifest.spriteVersionNumber ?? 1;
  if (version !== 1 && version !== 2) throw new Error("spriteVersionNumber must be 1, 2, or omitted.");
  const spriteName = requiredText(manifest.spritesheetPath, "spritesheet.webp", 120);
  if (!/^[A-Za-z0-9._-]+\.(?:webp|png)$/i.test(spriteName)) throw new Error("spritesheetPath must name a WebP or PNG beside pet.json.");
  const base = manifestPath.includes("/") ? manifestPath.slice(0, manifestPath.lastIndexOf("/") + 1) : "";
  const spritePath = [...normalized.keys()].find((path) => path.toLowerCase() === `${base}${spriteName}`.toLowerCase());
  if (!spritePath) throw new Error("The spritesheet referenced by pet.json is missing.");
  const spriteBytes = normalized.get(spritePath) as Uint8Array;
  const spriteMime = spriteName.toLowerCase().endsWith(".png") ? "image/png" : "image/webp";
  const fallbackId = base.replace(/\/$/, "").split("/").at(-1) || "imported-pet";
  const id = requiredText(manifest.id, fallbackId, 80);
  return {
    id,
    displayName: requiredText(manifest.displayName, id, 80),
    description: requiredText(manifest.description, "A custom Codex pet.", 240),
    spriteVersionNumber: version,
    spritesheetPath: spriteName,
    spriteBytes,
    spriteMime,
  };
}

export async function validateSpriteImage(pet: ParsedPetPackage): Promise<void> {
  const copy = new Uint8Array(pet.spriteBytes.byteLength);
  copy.set(pet.spriteBytes);
  const blob = new Blob([copy], { type: pet.spriteMime });
  let image: ImageBitmap;
  try { image = await createImageBitmap(blob); }
  catch { throw new Error("The pet spritesheet could not be decoded."); }
  try {
    const expectedHeight = pet.spriteVersionNumber === 2 ? 2288 : 1872;
    if (image.width !== 1536 || image.height !== expectedHeight) {
      throw new Error(`This v${pet.spriteVersionNumber} spritesheet must be 1536×${expectedHeight} pixels.`);
    }
    const canvas = document.createElement("canvas");
    canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("The spritesheet could not be inspected.");
    context.drawImage(image, 0, 0);
    const rgba = context.getImageData(0, 0, image.width, image.height).data;
    let hasTransparency = false;
    for (let index = 3; index < rgba.length; index += 4) if ((rgba[index] ?? 255) < 255) { hasTransparency = true; break; }
    if (!hasTransparency) throw new Error("The pet spritesheet must contain transparency.");
  } finally { image.close(); }
}

export async function parsePetPackage(file: File): Promise<ParsedPetPackage> {
  if (!/\.(?:zip|codex-pet\.zip)$/i.test(file.name)) throw new Error("Choose a Codex pet ZIP package.");
  const pet = await parsePetArchive(new Uint8Array(await file.arrayBuffer()));
  await validateSpriteImage(pet);
  return pet;
}
