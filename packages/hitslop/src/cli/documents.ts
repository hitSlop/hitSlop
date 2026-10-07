import type { EngineMethod, EngineRequestFor, EngineSuccess } from "../wire/engine";
// Document commands. Each is one `SocketRequest` the document engine sends to the
// document's live owner, or to an owner it opens; files the command names are read and
// written here.
import { lstat, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { AttachmentLimits, SocketLimits, ThemeFileLimit, type ExportFormats } from "../schema/constants";
import type { SocketFailure } from "../wire/socket";
import type { OutcomeCode } from "../schema/values";
import type { PaletteIntent } from "../schema/core";

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

/** A refusal in the core's words, with the operation it refused (`ops[N]`, an index into
 * the batch) and its reason code, so an agent knows what to fix. */
function refusal({ error, reason, opIndex }: SocketFailure, remedy: (message: string) => string) {
  const code = reason ? ` (${reason})` : "";
  return opIndex === undefined ? `${(reason === "requires_update" ? remedy(error) : error)}${code}` : `Refused ops[${opIndex}]${code}: ${(reason === "requires_update" ? remedy(error) : error)}`;
}
/** One request, and its successful reply's result as the method's contract requires it.
 * A failed edit says what it means for the next one. */
async function send<M extends EngineMethod>(body: EngineRequestFor<M> & { method: M }): Promise<Extract<EngineSuccess, { method: M }>> {
  const { request, withRemedy } = await import("./engine");
  const reply = await request<M>(body);
  if (!reply.ok) throw new Error([refusal(reply, withRemedy), body.method === "batch" || body.method === "call" ? outcomes[reply.code] : undefined].filter(Boolean).join("\n"));
  return reply as Extract<EngineSuccess, { method: M }>;
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
  const { state } = await send({ method: "get", ...at(document) });
  print(snapshot ? state : state.value);
}

export async function call(document: string, command: string, args: string) {
  const parsed = json(args);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("--args must be a JSON object");
  const input = { ...at(document), command, args: parsed };
  const reply = await send({ method: "call", ...input });
  print({ result: reply.result, ids: reply.ids });
}
export async function describe(document: string, machine = false) {
  const { state } = await send({ method: "describe", ...at(document) });
  if (machine) return print(state);
  const lines = [`${state.metadata.title}: ${state.metadata.description}`, `Version: ${state.version}`, "", "Fields"];
  for (const field of state.fields)
    lines.push(`  ${JSON.stringify(field.path)}: ${field.kind} (${field.operations.join(", ")})${field.description ? " — " + field.description : ""}`);
  lines.push("", "Commands (slop call PATH NAME --args JSON)");
  for (const [name, spec] of Object.entries(state.commands) as [string, { description: string; args: unknown }][]) {
    lines.push(`  ${name} — ${spec.description}`, `    args: ${JSON.stringify(spec.args)}`);
  }
  if (!Object.keys(state.commands).length) lines.push("  None. Use slop apply or slop batch.");
  lines.push("", "Value (rows retain their $id)", JSON.stringify(state.value, null, 2));
  console.log(lines.join("\n"));
}
/** An atomic batch. `ops` stays the text given, so numbers keep their spelling. Its text
 * sets merge from `base` when given. The `attach` files are stored in the same request,
 * before the operations that reference them (`attachmentsRef` prints a reference). */
export async function batch(document: string, ops: string, base?: string, attach: string[] = []) {
  if (!Array.isArray(json(ops))) throw new Error("--ops must be a JSON array of operations");
  const attachments = await Promise.all(attach.map(async (file) => Buffer.from((await reference(file)).bytes).toString("base64")));
  if (attachments.reduce((total, encoded) => total + encoded.length, ops.length) > SocketLimits.attachment - 4096)
    throw new Error(`Attachments exceed ${SocketLimits.attachment >> 20} MiB in one batch; attach fewer files per batch`);
  const { ids } = await send({
    method: "batch",
    ...at(document),
    ops,
    ...(base === undefined ? {} : { base }),
    ...(attachments.length ? { attachments } : {}),
  });
  print({ ids });
}
export async function apply(document: string, op: string, base?: string, attach: string[] = []) {
  const value = json(op);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("--op must be one JSON object");
  await batch(document, `[${op}]`, base, attach);
}
/** One `replace`: the value at `path` (the whole document by default) becomes the file's
 * JSON. Both are spliced as written; the core parses and validates the result. */
export async function importValue(document: string, file: string, path = "[]") {
  const value = text(await read(file, SocketLimits.request - 4096), basename(file));
  if (json(value) === undefined) throw new Error("The file must hold one JSON value");
  if (!Array.isArray(json(path))) throw new Error(`--path must be a JSON array, such as '["rows"]'`);
  await batch(document, `[{"type":"replace","path":${path},"value":${value}}]`);
}
export async function exportDocument(document: string, format: ExportFormat, output: string) {
  console.log((await send({ method: "export", ...at(document), format, output: resolve(output) })).output);
}

/** The palette: the template's colors, the document's overrides and the result. */
async function palette(document: string) {
  const { defaults, theme: effective } = (await send({ method: "get", ...at(document) })).state;
  // A write drops an override equal to its default, so the overrides are the colors that differ.
  const overrides = Object.fromEntries(Object.entries(effective).filter(([token, color]) => defaults[token] !== color));
  return { defaults, overrides, effective };
}
/** One palette intent, as an atomic batch, then the palette it left. */
async function changeTheme(document: string, intent: PaletteIntent) {
  await send({ method: "batch", ...at(document), ops: JSON.stringify([intent]) });
  print(await palette(document));
}
export async function themeGet(document: string) {
  print(await palette(document));
}
export async function themeSet(document: string, values: string) {
  const parsed = json(values);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed) || !Object.values(parsed).every((v) => typeof v === "string"))
    throw new Error("--values must be a JSON object of theme tokens and colors");
  await changeTheme(document, { type: "setTheme", values: parsed as Record<string, string> });
}
export async function themeReset(document: string, token?: string) {
  await changeTheme(document, { type: "setTheme", values: token === undefined ? {} : { [token]: null }, replace: token === undefined });
}
/** The theme file as the core writes it, to `output` or standard output. */
export async function themeExport(document: string, output?: string) {
  const { file } = (await send({ method: "theme.export", ...at(document) })).state;
  if (output === undefined) return process.stdout.write(file);
  await publish(output, file);
}
export async function themeImport(document: string, file: string) {
  const contents = text(await read(file, ThemeFileLimit), "Theme file");
  await changeTheme(document, { type: "importTheme", file: contents });
}

export async function attachmentsList(document: string) {
  print((await send({ method: "attachments.list", ...at(document) })).state);
}
/** A file as an attachment: its bytes, and the reference a document's operations store:
 * its identity (the SHA-256 of its bytes), length, name and type. */
async function reference(file: string) {
  const name = basename(file);
  const mimeType = Bun.file(file).type.split(";")[0] || "application/octet-stream";
  const length = (s: string) => new TextEncoder().encode(s).length;
  if (length(name) > AttachmentLimits.name || length(mimeType) > AttachmentLimits.name)
    throw new Error(`File name or type exceeds ${AttachmentLimits.name} bytes`);
  const bytes = await read(file, AttachmentLimits.file);
  const id = new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
  return { bytes, ref: { id, byteLength: bytes.length, name, mimeType } };
}
/** Prints a file's reference without reading any document: operations name it, and
 * `--attach` stores the file in their batch. */
export async function attachmentsRef(file: string) {
  print((await reference(file)).ref);
}
/** Writes an attachment's bytes to `output`. */
export async function attachmentsExport(document: string, id: string, output: string) {
  const { bytes } = (await send({ method: "attachments.read", ...at(document), attachmentID: id })).state;
  await publish(output, Buffer.from(bytes, "base64"));
}
