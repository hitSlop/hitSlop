import JSZip from "jszip";
const CLASSIC_BITMAPS = new Set([
  "balance.bmp", "cbuttons.bmp", "eq_ex.bmp", "eqmain.bmp", "gen.bmp", "genex.bmp", "main.bmp",
  "monoster.bmp", "numbers.bmp", "nums_ex.bmp", "playpaus.bmp", "pledit.bmp", "posbar.bmp", "shufrep.bmp",
  "text.bmp", "titlebar.bmp", "video.bmp", "volume.bmp",
]);

const u16 = (bytes: Uint8Array, at: number): number | null => at >= 0 && at + 2 <= bytes.length ? (bytes[at] ?? 0) | (bytes[at + 1] ?? 0) << 8 : null;
const u32 = (bytes: Uint8Array, at: number): number | null => {
  if (at < 0 || at + 4 > bytes.length) return null;
  return ((bytes[at] ?? 0) | (bytes[at + 1] ?? 0) << 8 | (bytes[at + 2] ?? 0) << 16 | (bytes[at + 3] ?? 0) << 24) >>> 0;
};

export async function validateClassicSkin(file: File): Promise<void> {
  if (!/\.(wsz|zip)$/i.test(file.name)) throw new Error("Choose a classic Winamp .wsz skin.");
  if (file.size > 10 * 1024 * 1024) throw new Error("That skin is larger than 10 MB.");
  const bytes = new Uint8Array(await file.arrayBuffer());
  const first = Math.max(0, bytes.length - 22 - 65_535), last = bytes.length - 22;
  let end = -1;
  for (let at = last; at >= first; at -= 1) if (u32(bytes, at) === 0x06054b50) { end = at; break; }
  if (end < 0) throw new Error("That file is not a readable ZIP skin.");
  const count = u16(bytes, end + 10), centralSize = u32(bytes, end + 12), centralOffset = u32(bytes, end + 16);
  if (count == null || centralSize == null || centralOffset == null || count < 1 || count > 256 || centralOffset + centralSize > end) throw new Error("That skin archive is malformed or too large.");
  let cursor: number = centralOffset;
  let total = 0, hasClassicBitmap = false, hasModernManifest = false;
  for (let index = 0; index < count; index += 1) {
    if (u32(bytes, cursor) !== 0x02014b50) throw new Error("That skin archive is malformed.");
    const flags = u16(bytes, cursor + 8), method = u16(bytes, cursor + 10), compressed = u32(bytes, cursor + 20), size = u32(bytes, cursor + 24);
    const nameLength = u16(bytes, cursor + 28), extraLength = u16(bytes, cursor + 30), commentLength = u16(bytes, cursor + 32);
    if (flags == null || method == null || compressed == null || size == null || nameLength == null || extraLength == null || commentLength == null) throw new Error("That skin archive is truncated.");
    if ((flags & 1) !== 0) throw new Error("Encrypted skins are not supported.");
    if (![0, 8].includes(method)) throw new Error("That skin uses unsupported ZIP compression.");
    if (compressed === 0xffff_ffff || size === 0xffff_ffff) throw new Error("ZIP64 skins are not supported.");
    if (size > 25 * 1024 * 1024) throw new Error("A file inside that skin is too large.");
    total += size; if (total > 50 * 1024 * 1024) throw new Error("That skin expands beyond 50 MB.");
    const nameStart: number = cursor + 46, next: number = nameStart + nameLength + extraLength + commentLength;
    if (next > bytes.length) throw new Error("That skin archive is truncated.");
    const path = new TextDecoder().decode(bytes.subarray(nameStart, nameStart + nameLength));
    if (/^[\\/]|^[a-z]:/i.test(path) || path.split(/[\\/]/).some(part => part === ".."))
      throw new Error("Skin archive paths must stay inside the skin.");
    const name = path.split(/[\\/]/).at(-1)?.toLowerCase() ?? "";
    if (name === "skin.xml") hasModernManifest = true;
    if (CLASSIC_BITMAPS.has(name) || CLASSIC_BITMAPS.has(name.replace(/\.png$/, ".bmp"))) hasClassicBitmap = true;
    cursor = next;
  }
  if (hasModernManifest) throw new Error("Modern Winamp skins are not supported; choose a classic .wsz skin.");
  if (!hasClassicBitmap) throw new Error("No classic Winamp skin bitmaps were found.");
  // Check actual inflation as well as the ZIP directory's untrusted size claims.
  const zip = await JSZip.loadAsync(bytes);
  let expanded = 0;
  for (const entry of Object.values(zip.files)) {
    if (entry.dir) continue;
    let length = 0;
    await new Promise<void>((resolve, reject) => {
      const stream = (entry as typeof entry & { internalStream(type: "uint8array"): JSZip.JSZipStreamHelper<Uint8Array> }).internalStream("uint8array");
      stream.on("data", chunk => {
        length += chunk.length; expanded += chunk.length;
        if (length > 25 * 1024 * 1024 || expanded > 50 * 1024 * 1024) {
          stream.pause(); reject(new Error("That skin expands beyond its size limit."));
        }
      }).on("error", reject).on("end", resolve).resume();
    });
  }
}
