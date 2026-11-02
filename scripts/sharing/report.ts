/** Failure reports keep diagnostics without exposing a private invitation. */
export function sharingFailureMessage(error: unknown): string {
  return String(error).replace(/https?:\/\/[^\s/]+\/join#[a-f0-9]+\.[a-f0-9]+/gi, "[private invitation]");
}
