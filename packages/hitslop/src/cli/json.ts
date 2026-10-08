/** Parse untrusted CLI JSON without assigning a payload type or losing JSON null. */
export function json(text: string): unknown {
  try { return JSON.parse(text); }
  catch { return undefined; }
}

// Bun's source-aware reviver preserves numeric spelling until Rust accepts the batch.
// These runtime extensions predate their declarations in the workspace TS library.
declare global {
  interface JSON {
    parse(text: string, reviver: (key: string, value: unknown, context: { source?: string }) => unknown): unknown;
    rawJSON(text: string): unknown;
  }
}
