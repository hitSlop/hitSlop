import { json, print } from "./json";
import { execute as send } from "./engine";
// Document commands. Each is one `SocketRequest` the document engine sends to the
// document's live owner, or to an owner it opens; files the command names are read and
// written here.
import { lstat, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { AttachmentLimits, SocketLimits, ThemeFileLimit, type ExportFormats } from "../schema/constants";
import type { PaletteIntent, Batch } from "../schema/core";

type ExportFormat = (typeof ExportFormats)[number];
/** The document a request names. */
const at = (document: string) => ({ documentPath: resolve(document) });
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
/** Format the core's command schema for people; machine output keeps the full projection. */
function argumentLines(schema: Record<string, unknown>): string[] {
  const properties = Object.entries(schema.properties as Record<string, Record<string, unknown>>);
  if (!properties.length) return ["    No arguments. Pass --args '{}'."];
  const required = new Set(schema.required as string[] | undefined);
  return properties.map(([name, argument]) => {
    const details = [required.has(name) ? "required" : "optional"];
    if (Object.hasOwn(argument, "default")) details.push(`default: ${JSON.stringify(argument.default)}`);
    if (Array.isArray(argument.enum)) details.push(`choices: ${argument.enum.map(value => JSON.stringify(value)).join(", ")}`);
    for (const [key, label] of [["minimum", "min"], ["maximum", "max"], ["minLength", "min length"], ["maxLength", "max length"]] as const)
      if (argument[key] !== undefined) details.push(`${label}: ${argument[key]}`);
    return `    ${name}: ${argument.type} (${details.join("; ")})${argument.description ? " — " + argument.description : ""}`;
  });
}

export async function describe(document: string, machine = false) {
  const { state } = await send({ method: "describe", ...at(document) });
  if (machine) return print(state);
  const lines = [`${state.metadata.title}: ${state.metadata.description}`, `Version: ${state.version}`, "", "Fields"];
  for (const field of state.fields)
    lines.push(`  ${JSON.stringify(field.path)}: ${field.kind} (${field.operations.join(", ")})${field.description ? " — " + field.description : ""}`);
  lines.push("", "Commands (slop call PATH NAME --args JSON)");
  if (Object.keys(state.commands).length) lines.push("  Arguments accept only the listed fields.");
  for (const [name, spec] of Object.entries(state.commands)) {
    lines.push("", `  ${name} — ${spec.description}`, ...argumentLines(spec.args));
  }
  if (!Object.keys(state.commands).length) lines.push("  None. Use slop apply or slop batch.");
  lines.push("", "Value (rows retain their $id)", JSON.stringify(state.value, null, 2));
  console.log(lines.join("\n"));
}
/** Preserve numeric tokens until Rust validates them; JavaScript must not round CLI input. */
export function batchIntents(ops: string): Batch["intents"] {
  // Bun supports source-aware revivers and rawJSON; the workspace TS lib predates them.
  const intents = JSON.parse(ops, (_key, value, context) => {
    if (typeof value !== "number") return value;
    if (context.source === undefined) throw new Error("Bun must support source-aware JSON parsing");
    return JSON.rawJSON(context.source);
  });
  if (!Array.isArray(intents)) throw new Error("--ops must be a JSON array of operations");
  // Shape and value acceptance belong to Rust, including raw numeric tokens.
  return intents as Batch["intents"];
}
/** An atomic batch. Numeric tokens keep their spelling. A text set with `from` merges with
 * edits made since the agent read the field. The `attach` files are stored in the same
 * request, before the operations that reference them (`attachmentsRef` prints a reference). */
export async function batch(document: string, ops: string, attach: string[] = []) {
  const intents = batchIntents(ops);
  const attachments = await Promise.all(attach.map(async (file) => Buffer.from(await read(file, AttachmentLimits.file)).toString("base64")));
  if (attachments.reduce((total, encoded) => total + encoded.length, ops.length) > SocketLimits.attachment - 4096)
    throw new Error(`Attachments exceed ${SocketLimits.attachment >> 20} MiB in one batch; attach fewer files per batch`);
  const { ids } = await send({
    method: "batch",
    ...at(document),
    batch: { intents },
    ...(attachments.length ? { attachments } : {}),
  });
  print({ ids });
}
export async function apply(document: string, ops: string[], attach: string[] = []) {
  // The flag is repeatable only so a second --op is refused instead of silently replacing the first.
  if (ops.length !== 1) throw new Error("slop apply takes one --op; pass several operations to slop batch --ops");
  const [op] = ops;
  const value = json(op);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("--op must be one JSON object");
  await batch(document, `[${op}]`, attach);
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
  await send({ method: "batch", ...at(document), batch: { intents: [intent] } });
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
/** Prints a file's reference without reading any document: operations name it, and
 * `--attach` stores the file in their batch. */
export async function attachmentsRef(file: string) {
  print((await send({ method: "attachments.ref", file: resolve(file) })).reference);
}
/** Writes an attachment's bytes to `output`. */
export async function attachmentsExport(document: string, id: string, output: string) {
  const { bytes } = (await send({ method: "attachments.read", ...at(document), attachmentID: id })).state;
  await publish(output, Buffer.from(bytes, "base64"));
}
