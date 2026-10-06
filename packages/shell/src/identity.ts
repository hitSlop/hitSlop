import { RowIdRule } from "@hitslop/schema/constants";
/** Row identity is an application register (`$id`), independent of Loro container IDs. */
const alphabet = RowIdRule.mintAlphabet;
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
