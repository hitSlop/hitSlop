/** Failure reports keep diagnostics without exposing a private invitation: everything after
 * `/join#` (the room and its capability) is the secret, whatever its format. */
export function sharingFailureMessage(error: unknown): string {
  return String(error).replace(/https?:\/\/[^\s/]+\/join#[^\s"'<>()]+/gi, "[private invitation]");
}
