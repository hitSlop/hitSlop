import { RowIdRule } from "../schema/constants";
/** Row identity is an application register (`$id`), independent of Loro container IDs. */
const alphabet = RowIdRule.mintAlphabet;
/** 128 random bits as 26 Crockford base32 characters; `fill` supplies the bits. */
export function newID(fill: (bytes: Uint8Array) => Uint8Array = (bytes) => crypto.getRandomValues(bytes)): string {
  const bytes = fill(new Uint8Array(16));
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
