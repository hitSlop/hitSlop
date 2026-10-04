// Document commands. Each is one `SocketRequest` the macOS helper sends to the document's
// live owner, or to an owner it opens; files the command names are read and written here.
import { lstat, realpath, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { AttachmentLimits, SocketLimits, ThemeFileLimit } from "@hitslop/schema/constants";
import { SocketResults } from "@hitslop/schema/socket";
import { validate } from "@hitslop/schema/validation";
import type { OutcomeCode } from "@hitslop/schema/values";

/** What a failed edit means for the next one. */
const outcomes: Record<OutcomeCode, string> = {
  rejected: "Not applied.",
  owner_replaced: "Not applied. Run slop get before issuing another edit.",
  closing: "Not applied. Run slop get before issuing another edit.",
  owner_invalidated: "Not applied.",
  save_failed: "Applied, but not yet saved. Run slop get before issuing another edit.",
  unknown_outcome: "Outcome unknown. Run slop get before issuing another edit.",
};

/** One request, and its successful reply's result as the method's contract requires it. */
async function send<M extends keyof typeof SocketResults>(method: M, document: string, fields: Record<string, unknown> = {}, edit = false) {
  if (process.platform !== "darwin")
    throw new Error("Document commands and export require macOS and hitSlop.app; init, check, dev and build run anywhere.");
  const reply = await (await import("./native")).request({ method, documentPath: resolve(document), ...fields });
  if (!reply.ok) {
    const outcome = edit ? outcomes[reply.code ?? "unknown_outcome"] : undefined;
    throw new Error([reply.error ?? "Document operation failed", outcome].filter(Boolean).join("\n"));
  }
  return validate(SocketResults[method], reply, `hitSlop.app sent an invalid ${method} reply; outcome unknown, run slop get before another edit`);
}
const print = (value: unknown) => process.stdout.write(JSON.stringify(value, null, 2) + "\n");
/** A regular file a command reads, at most `limit` bytes. */
async function read(path: string, limit: number): Promise<Uint8Array> {
  const info = await lstat(path).catch(() => undefined);
  if (!info?.isFile()) throw new Error(`${basename(path)} must be a regular file`);
  if (info.size > limit) throw new Error(`${basename(path)} exceeds ${limit} bytes`);
  return new Uint8Array(await Bun.file(path).arrayBuffer());
}
function text(bytes: Uint8Array, what: string) {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new Error(`${what} must be UTF-8`);
  }
}
const json = (value: string): unknown => {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
};

export async function get(document: string, snapshot: boolean) {
  const { schema, state: frame } = (await send("get", document)).state;
  print(snapshot ? { schema, state: frame } : frame.value);
}
/** An atomic batch. `ops` stays the text given, so numbers keep their spelling. */
export async function batch(document: string, ops: string) {
  if (!Array.isArray(json(ops))) throw new Error("--ops must be a JSON array of operations");
  const { ids, sequence } = await send("batch", document, { ops }, true);
  print({ ids, sequence });
}
export async function apply(document: string, op: string) {
  const value = json(op);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("--op must be one JSON object");
  await batch(document, `[${op}]`);
}
/** One `replace`: the value at `path` (the whole document by default) becomes the file's
 * JSON. Both are spliced as written; the core parses and validates the result. */
export async function importValue(document: string, file: string, path = "[]") {
  const value = text(await read(file, SocketLimits.request - 4096), basename(file));
  if (json(value) === undefined) throw new Error("The file must hold one JSON value");
  if (!Array.isArray(json(path))) throw new Error(`--path must be a JSON array, such as '["rows"]'`);
  await batch(document, `[{"type":"replace","path":${path},"value":${value}}]`);
}
export async function compact(document: string) {
  await send("compact", document, {}, true);
}
export async function exportDocument(document: string, format: string, output: string) {
  console.log((await send("export", document, { format, output: resolve(output) })).output);
}

export async function themeGet(document: string) {
  print((await send("theme.get", document)).state);
}
export async function themeSet(document: string, values: string) {
  const parsed = json(values);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed) || !Object.values(parsed).every((v) => typeof v === "string"))
    throw new Error("--values must be a JSON object of theme tokens and colors");
  print((await send("theme.set", document, { values: parsed }, true)).state);
}
export async function themeReset(document: string, token?: string) {
  print((await send("theme.reset", document, token === undefined ? {} : { token }, true)).state);
}
/** The theme file as the core writes it, to `output` or standard output. */
export async function themeExport(document: string, output?: string) {
  const { file } = (await send("theme.export", document)).state;
  if (output === undefined) return process.stdout.write(file);
  await writeFile(output, file);
  console.log(resolve(output));
}
export async function themeImport(document: string, file: string) {
  const contents = text(await read(file, ThemeFileLimit), "Theme file");
  print((await send("theme.import", document, { file: contents }, true)).state);
}

export async function attachmentsList(document: string) {
  print((await send("attachments.list", document)).state);
}
/** Saves a file and prints its reference: the stored identity, the file's name and type. */
export async function attachmentsImport(document: string, file: string) {
  const name = basename(file);
  const mimeType = Bun.file(file).type.split(";")[0] || "application/octet-stream";
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  if (bytes(name) > AttachmentLimits.name || bytes(mimeType) > AttachmentLimits.name)
    throw new Error(`File name or type exceeds ${AttachmentLimits.name} bytes`);
  const encoded = Buffer.from(await read(file, AttachmentLimits.file)).toString("base64");
  const stored = (await send("attachments.put", document, { bytes: encoded }, true)).state;
  print({ ...stored, name, mimeType });
}
/** Writes an attachment's bytes to `output`, never over an existing file or the document. */
export async function attachmentsExport(document: string, id: string, output: string) {
  const destination = join(await realpath(dirname(resolve(output))), basename(output));
  if (destination === (await realpath(resolve(document)).catch(() => resolve(document))))
    throw new Error("Export destination must not be the document");
  const { bytes } = (await send("attachments.read", document, { attachmentID: id })).state;
  await writeFile(destination, Buffer.from(bytes, "base64"), { flag: "wx" });
  console.log(destination);
}
