// Document commands. Each is one `SocketRequest` the macOS helper sends to the document's
// live owner, or to an owner it opens; files the command names are read and written here.
import { lstat, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { AttachmentLimits, SocketLimits, ThemeFileLimit, type ExportFormats } from "@hitslop/schema/constants";
import { EpochMethods, SocketResults, type HelperRequestFor, type SocketMethod } from "@hitslop/schema/socket";
import { validate } from "@hitslop/schema/validation";
import type { OutcomeCode } from "@hitslop/schema/values";

type ExportFormat = (typeof ExportFormats)[number];
/** What a failed edit means for the next one. */
const outcomes: Record<OutcomeCode, string> = {
  rejected: "Not applied.",
  owner_replaced: "Not applied. Run slop get before issuing another edit.",
  closing: "Not applied. Run slop get before issuing another edit.",
  owner_invalidated: "Not applied.",
  save_failed: "Applied, but not yet saved. Run slop get before issuing another edit.",
  unknown_outcome: "Outcome unknown. Run slop get before issuing another edit.",
};

/** One request, and its successful reply's result as the method's contract requires it.
 * A failed request that carries the owner's epoch says what it means for the next edit. */
async function send<M extends Exclude<SocketMethod, "hello">>(request: HelperRequestFor<M> & { method: M }) {
  const { ExitStatus, request: helper } = await import("./native");
  const outcome = (code: OutcomeCode) => (EpochMethods.has(request.method) ? outcomes[code] : undefined);
  const reply = await helper(request).catch((error) => {
    throw error instanceof ExitStatus ? new ExitStatus(error.code, outcome("unknown_outcome")) : error;
  });
  if (!reply.ok) throw new Error([reply.error ?? "Document operation failed", outcome(reply.code ?? "unknown_outcome")].filter(Boolean).join("\n"));
  const { method } = request;
  return validate(SocketResults[method], reply, `hitSlop.app sent an invalid ${method} reply; outcome unknown, run slop get before another edit`);
}
/** The document a request names. */
const at = (document: string) => ({ documentPath: resolve(document) });
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
/** Writes an export to `output`. Exports never replace a file: not the document, another
 * document or an earlier export, under any spelling of its path. */
async function publish(output: string, bytes: string | Uint8Array) {
  await writeFile(output, bytes, { flag: "wx" }).catch((error: NodeJS.ErrnoException) => {
    throw error.code === "EEXIST" ? new Error(`${output} already exists; exports never replace a file`) : error;
  });
  console.log(resolve(output));
}
const json = (value: string): unknown => {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
};

export async function get(document: string, snapshot: boolean) {
  const { schema, state: frame } = (await send({ method: "get", ...at(document) })).state;
  print(snapshot ? { schema, state: frame } : frame.value);
}
/** An atomic batch. `ops` stays the text given, so numbers keep their spelling. */
export async function batch(document: string, ops: string) {
  if (!Array.isArray(json(ops))) throw new Error("--ops must be a JSON array of operations");
  const { ids, sequence } = await send({ method: "batch", ...at(document), ops });
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
  await send({ method: "compact", ...at(document) });
}
export async function exportDocument(document: string, format: ExportFormat, output: string) {
  console.log((await send({ method: "export", ...at(document), format, output: resolve(output) })).output);
}

export async function themeGet(document: string) {
  print((await send({ method: "theme.get", ...at(document) })).state);
}
export async function themeSet(document: string, values: string) {
  const parsed = json(values);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed) || !Object.values(parsed).every((v) => typeof v === "string"))
    throw new Error("--values must be a JSON object of theme tokens and colors");
  print((await send({ method: "theme.set", ...at(document), values: parsed as Record<string, string> })).state);
}
export async function themeReset(document: string, token?: string) {
  print((await send({ method: "theme.reset", ...at(document), ...(token === undefined ? {} : { token }) })).state);
}
/** The theme file as the core writes it, to `output` or standard output. */
export async function themeExport(document: string, output?: string) {
  const { file } = (await send({ method: "theme.export", ...at(document) })).state;
  if (output === undefined) return process.stdout.write(file);
  await publish(output, file);
}
export async function themeImport(document: string, file: string) {
  const contents = text(await read(file, ThemeFileLimit), "Theme file");
  print((await send({ method: "theme.import", ...at(document), file: contents })).state);
}

export async function attachmentsList(document: string) {
  print((await send({ method: "attachments.list", ...at(document) })).state);
}
/** Saves a file and prints its reference: the stored identity, the file's name and type. */
export async function attachmentsImport(document: string, file: string) {
  const name = basename(file);
  const mimeType = Bun.file(file).type.split(";")[0] || "application/octet-stream";
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  if (bytes(name) > AttachmentLimits.name || bytes(mimeType) > AttachmentLimits.name)
    throw new Error(`File name or type exceeds ${AttachmentLimits.name} bytes`);
  const encoded = Buffer.from(await read(file, AttachmentLimits.file)).toString("base64");
  const stored = (await send({ method: "attachments.put", ...at(document), bytes: encoded })).state;
  print({ ...stored, name, mimeType });
}
/** Writes an attachment's bytes to `output`. */
export async function attachmentsExport(document: string, id: string, output: string) {
  const { bytes } = (await send({ method: "attachments.read", ...at(document), attachmentID: id })).state;
  await publish(output, Buffer.from(bytes, "base64"));
}
