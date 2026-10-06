import { describe, expect, test } from "bun:test";
import { parsePetArchive } from "./pet-package";

const text = new TextEncoder();

function crc32(bytes: Uint8Array): number {
  let crc = ~0;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return ~crc >>> 0;
}

/** Stored-method ZIP so archive checks see the paths exactly as authored. */
function zip(files: Record<string, Uint8Array>): Uint8Array {
  const locals: Uint8Array[] = [];
  const centrals: Uint8Array[] = [];
  let offset = 0;
  for (const [name, data] of Object.entries(files)) {
    const nameBytes = text.encode(name);
    const checksum = crc32(data);
    const local = new Uint8Array(30 + nameBytes.length + data.length);
    const view = new DataView(local.buffer);
    view.setUint32(0, 0x04034b50, true);
    view.setUint32(14, checksum, true);
    view.setUint32(18, data.length, true);
    view.setUint32(22, data.length, true);
    view.setUint16(26, nameBytes.length, true);
    local.set(nameBytes, 30);
    local.set(data, 30 + nameBytes.length);
    locals.push(local);
    const central = new Uint8Array(46 + nameBytes.length);
    const directory = new DataView(central.buffer);
    directory.setUint32(0, 0x02014b50, true);
    directory.setUint32(16, checksum, true);
    directory.setUint32(20, data.length, true);
    directory.setUint32(24, data.length, true);
    directory.setUint16(28, nameBytes.length, true);
    directory.setUint32(42, offset, true);
    central.set(nameBytes, 46);
    centrals.push(central);
    offset += local.length;
  }
  const centralSize = centrals.reduce((sum, part) => sum + part.length, 0);
  const end = new Uint8Array(22);
  const tail = new DataView(end.buffer);
  tail.setUint32(0, 0x06054b50, true);
  tail.setUint16(8, centrals.length, true);
  tail.setUint16(10, centrals.length, true);
  tail.setUint32(12, centralSize, true);
  tail.setUint32(16, offset, true);
  const out = new Uint8Array(offset + centralSize + end.length);
  let cursor = 0;
  for (const part of [...locals, ...centrals, end]) {
    out.set(part, cursor);
    cursor += part.length;
  }
  return out;
}

const manifest = (extra: Record<string, unknown> = {}) => text.encode(JSON.stringify({
  id: "tiny-pet",
  displayName: "Tiny Pet",
  description: "A fixture pet.",
  spritesheetPath: "spritesheet.webp",
  ...extra,
}));

describe("Codex pet ZIP parsing", () => {
  test("accepts a root v1 package and harmless preview files", async () => {
    const parsed = await parsePetArchive(zip({
      "pet.json": manifest(),
      "spritesheet.webp": new Uint8Array([1, 2, 3]),
      "preview.webp": new Uint8Array([4]),
    }));
    expect(parsed.displayName).toBe("Tiny Pet");
    expect(parsed.spriteVersionNumber).toBe(1);
    expect([...parsed.spriteBytes]).toEqual([1, 2, 3]);
  });

  test("accepts a v2 package inside one wrapper directory", async () => {
    const parsed = await parsePetArchive(zip({
      "tiny/pet.json": manifest({ spriteVersionNumber: 2 }),
      "tiny/spritesheet.webp": new Uint8Array([9]),
    }));
    expect(parsed.spriteVersionNumber).toBe(2);
    expect(parsed.id).toBe("tiny-pet");
  });

  test("rejects missing, duplicate, and unsafe package paths", async () => {
    await expect(parsePetArchive(zip({ "spritesheet.webp": new Uint8Array([1]) }))).rejects.toThrow("exactly one pet.json");
    await expect(parsePetArchive(zip({
      "a/pet.json": manifest(), "a/spritesheet.webp": new Uint8Array([1]),
      "b/pet.json": manifest(), "b/spritesheet.webp": new Uint8Array([1]),
    }))).rejects.toThrow("exactly one pet.json");
    await expect(parsePetArchive(zip({ "../pet.json": manifest(), "../spritesheet.webp": new Uint8Array([1]) }))).rejects.toThrow("unsafe path");
  });

  test("rejects bad versions and escaping sprite references", async () => {
    await expect(parsePetArchive(zip({
      "pet.json": manifest({ spriteVersionNumber: 3 }), "spritesheet.webp": new Uint8Array([1]),
    }))).rejects.toThrow("spriteVersionNumber");
    await expect(parsePetArchive(zip({
      "pet.json": manifest({ spritesheetPath: "../spritesheet.webp" }), "spritesheet.webp": new Uint8Array([1]),
    }))).rejects.toThrow("beside pet.json");
  });

  test("rejects case-insensitive duplicate paths", async () => {
    await expect(parsePetArchive(zip({
      "pet.json": manifest(), "spritesheet.webp": new Uint8Array([1]), "SPRITESHEET.WEBP": new Uint8Array([2]),
    }))).rejects.toThrow("duplicate file paths");
  });
});
