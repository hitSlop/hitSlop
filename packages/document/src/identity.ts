/** Row and tree-node identity is an application register, independent of Loro container IDs. */
export const ID_KEY = "$id";
const alphabet = "0123456789abcdefghjkmnpqrstvwxyz";
/** 128 random bits as 26 Crockford base32 characters. */
export function newID(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  let bits = 0,
    buffer = 0,
    out = "";
  for (const byte of bytes) {
    buffer = (buffer << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += alphabet[(buffer >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  return out + alphabet[(buffer << (5 - bits)) & 31];
}
/** Accepted stored and imported IDs: short, printable, and safe inside JSON pointers. */
export const isID = (value: unknown): value is string =>
  typeof value === "string" && /^[0-9A-Za-z_-]{1,64}$/.test(value);

const fnv = (text: string, offset: bigint) => {
  let hash = offset;
  for (const byte of new TextEncoder().encode(text)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash;
};
/**
 * Deterministic ID for a row or tree node whose stored ID is missing, invalid or a
 * duplicate: "x-" and 24 base32 characters of a fixed 128-bit hash (two FNV-1a-64
 * passes) of its internal identity. Identical on every peer and runtime revision;
 * never written. Part of the data contract (golden vector in tests/open.test.ts).
 */
export function derivedID(internal: string): string {
  let bits = (fnv(internal, 0xcbf29ce484222325n) << 64n) | fnv("hitslop:" + internal, 0xcbf29ce484222325n);
  let out = "x-";
  for (let i = 0; i < 24; i++) {
    out += alphabet[Number(bits & 31n)];
    bits >>= 5n;
  }
  return out;
}
export type IdentityProblem = "Missing row ID" | "Invalid row ID" | "Duplicate row ID";
/**
 * Effective IDs for sibling rows (or all nodes of a tree). A valid stored ID belongs
 * to the row with the lowest internal identity among those claiming it, so moving
 * rows never transfers an ID. Every other row receives a derived ID that never
 * collides with a claimed one. Projection and write resolution share this rule, so
 * every visible row is addressable.
 */
export function effectiveIDs(entries: { stored: unknown; internal: string }[]) {
  const owners = new Map<string, string>();
  for (const { stored, internal } of entries)
    if (isID(stored) && !(owners.get(stored)! <= internal)) owners.set(stored, internal);
  const taken = new Set(owners.keys());
  return entries.map(({ stored, internal }) => {
    if (isID(stored) && owners.get(stored) === internal)
      return { id: stored as string, problem: undefined as IdentityProblem | undefined };
    const problem: IdentityProblem =
      stored === undefined ? "Missing row ID" : isID(stored) ? "Duplicate row ID" : "Invalid row ID";
    let id = derivedID(internal);
    for (let salt = 1; taken.has(id); salt++) id = derivedID(`${internal}#${salt}`);
    taken.add(id);
    return { id, problem };
  });
}
