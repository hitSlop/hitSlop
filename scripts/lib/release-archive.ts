import { gunzipSync } from "node:zlib";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

/** Our release archives use portable ustar with only directories and ordinary files.
 * Validate the entire archive before writing anything. Never extract links or paths
 * supplied by an archive through the system tar's filesystem operations. */
export function archiveFiles(compressed: Uint8Array): Map<string, Buffer> {
  const tar = gunzipSync(compressed, { maxOutputLength: 512 * 1024 * 1024 });
  const files = new Map<string, Buffer>();
  const seen = new Set<string>();
  const string = (header: Buffer, start: number, length: number) => header.subarray(start, start + length).toString("utf8").replace(/\0.*$/s, "");
  const octal = (value: string) => {
    if (!/^[0-7]+$/.test(value.trim())) throw new Error("Invalid tar integer");
    return Number.parseInt(value.trim(), 8);
  };
  let offset = 0;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every(byte => byte === 0)) {
      if (!tar.subarray(offset).every(byte => byte === 0)) throw new Error("Trailing archive data");
      return files;
    }
    const checksum = octal(string(header, 148, 8));
    const actual = [...header].reduce((sum, byte, index) => sum + (index >= 148 && index < 156 ? 32 : byte), 0);
    if (checksum !== actual) throw new Error("Invalid tar checksum");
    const prefix = string(header, 345, 155);
    const name = (prefix ? prefix + "/" : "") + string(header, 0, 100);
    const path = name === "./" ? "." : name.replace(/^\.\//, "").replace(/\/$/, "");
    const type = header[156];
    if (type !== 0 && type !== 48 && type !== 53) throw new Error("Release archives cannot contain links or special entries");
    if (path !== "." && (!path || path.includes("\\") || path.split("/").some(part => !part || part === ".." || part === ".")))
      throw new Error(`Unsafe archive path: ${name}`);
    if (seen.has(path)) throw new Error(`Duplicate archive path: ${path}`);
    seen.add(path);
    const size = octal(string(header, 124, 12));
    offset += 512;
    if (offset + size > tar.length || (type === 53 && size !== 0)) throw new Error("Truncated or invalid archive");
    if (type !== 53) {
      if (path === ".") throw new Error("Invalid archive file");
      files.set(path, tar.subarray(offset, offset + size));
    }
    offset += Math.ceil(size / 512) * 512;
  }
  throw new Error("Archive has no end marker");
}

/** Destination must be a newly created private directory, never a shared cache tree. */
export async function extractArchive(bytes: Uint8Array, destination: string) {
  for (const [name, content] of archiveFiles(bytes)) {
    const path = join(destination, name);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, content, { flag: "wx" });
  }
}
