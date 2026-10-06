import { AppLimits, AttachmentIdRule, CoreErrorCodes, OutcomeCodes, HelperProtocol, RowIdRule, AttachmentLimits, BatchLimits, DefaultWindowRadius, PackageFormat, PagePayloadLimit, AssetLimits, RuntimeABI, ShapeLimits, SocketLimits, StorageLimits, ThemeFileLimit, ThemeLimit, ThemeTokenRule } from "../../packages/hitslop/src/schema/constants";
import { SocketRequestSchema, SocketSuccessSchema, SocketFailureSchema } from "../../packages/hitslop/src/schema/socket";
import { EngineRequestSchema, EngineSuccessSchema } from "../../packages/hitslop/src/schema/engine";
import { PageRequestSchema } from "../../packages/hitslop/src/schema/page";
import { ThemeChangesSchema, ThemeFileSchema } from "../../packages/hitslop/src/schema/values";
import { AppRowSchema } from "../../packages/hitslop/src/schema/manifest";
import { variants, SegmentSchema as Segment, AnchorSchema as Anchor, SelectionSchema as Selection, TextHunkSchema as TextHunk, BatchSchema, OwnerPatchOpSchema as PatchOp, OwnerPublicationSchema } from "../../packages/hitslop/src/schema/core";
import type { AppAcceptance, StorageAcceptance } from "./acceptance";

// The deliberately small generator fails on unsupported types. It generates the
// Rust deserialization envelope; descriptor interpretation stays inside the core.
const upper = (s: string) => s[0]!.toUpperCase() + s.slice(1);
const pascal = (s: string) => s.split("_").map(upper).join("");
const optional = (s: object) => (s as any)["~optional"] === true;
function rust(schema: any): string {
  let result: string;
  if (schema === Anchor) result = "Anchor";
  else if (schema.type === "array")
    result = `Vec<${schema.items === Segment ? "Segment" : schema.items === TextHunk ? "Hunk" : rust(schema.items)}>`;
  else if (schema.type === "string") result = "String";
  else if (schema.type === "boolean") result = "bool";
  else if (schema === ThemeChangesSchema) result = "std::collections::BTreeMap<String, Option<String>>";
  else if (schema.type === "integer") result = schema.minimum >= 0 ? "usize" : "i64";
  else if (schema.anyOf && schema.anyOf[0]?.properties?.before) result = "Anchor";
  // `Type.Optional` copies the schema, so match the selection by its shape.
  else if (schema.type === "object" && Object.keys(schema.properties).join() === Object.keys(Selection.properties).join()) result = "Selection";
  else if (Object.keys(schema).length === 0) result = "Value";
  else throw new Error(`Unhandled wire type: ${JSON.stringify(schema)}`);
  return optional(schema) ? `Option<${result}>` : result;
}
const union = (name: string, schema: any, derive: string) => `#[derive(${derive})]
#[serde(untagged, deny_unknown_fields)]
pub enum ${name} {
${schema.anyOf
  .map((s: any) =>
    s.type === "string"
      ? "    Key(String),"
      : `    ${upper(Object.keys(s.properties)[0]!)} { ${Object.entries(s.properties)
          .map(([k, v]) => `${k}: ${rust(v)}`)
          .join(", ")} },`,
  )
  .join("\n")}
}`;
// Optional fields are omitted when absent.
const record = (name: string, schema: any, overrides: Record<string, string>, derive = "Debug, Serialize") => `#[derive(${derive})]
pub struct ${name} { ${Object.entries(schema.properties).map(([key, value]) => optional(value as object)
  ? `#[serde(skip_serializing_if = "Option::is_none")] pub ${key}: Option<${overrides[key]}>`
  : `pub ${key}: ${overrides[key] ?? rust(value)}`).join(", ")} }`;
const socketName = (value: string) => value.split(/[._-]/).map(upper).join("");
const socketFields = (schema: any) => Object.entries(schema.properties).filter(([key]) => key !== "method" && key !== "ok");
const socketRust = (key: string, schema: any) => {
  const value = key === "state" ? "Box<serde_json::value::RawValue>" : key === "values" ? "std::collections::BTreeMap<String, String>"
    : schema.type === "integer" ? "u64" : schema.type === "array" ? "Vec<String>" : "String";
  return optional(schema) ? `Option<${value}>` : value;
};
const requests = (union: { anyOf: any[] }) => union.anyOf.flatMap((schema) => {
  const method = schema.properties.method as { const?: string; enum?: string[] };
  return (method.enum ?? [method.const!]).map((method) => ({ method, fields: socketFields(schema) }));
});
const socketRequests = requests(SocketRequestSchema);
const socketEnum = (name: string, members: { method: string; fields: [string, unknown][] }[], derives: string, engine = false) => `
#[cfg(feature = "storage")]
#[derive(${derives})]
#[serde(tag = "method", deny_unknown_fields)]
#[allow(non_snake_case)]
pub${engine ? "" : "(crate)"} enum ${name} {
${members.map(({ method, fields }) => `    #[serde(rename = "${method}")]
    ${socketName(method)} { ${fields.map(([key, schema]) => `${optional(schema as object) ? '#[serde(skip_serializing_if = "Option::is_none")] ' : ""}${key}: ${engine ? engineRust(schema) : socketRust(key, schema)}`).join(", ")} },`).join("\n")}
}
`;
const engineRust = (schema: any): string => {
  const value = schema.type === "string" || schema.enum?.every((value: unknown) => typeof value === "string") ? "String" : schema.type === "boolean" ? "bool"
    : schema.type === "integer" ? "u64" : schema.type === "array" && schema.items.type === "string" ? "Vec<String>"
    : "Box<serde_json::value::RawValue>";
  return optional(schema) ? `Option<${value}>` : value;
};
export function rustOwnerWire(accepted: { app: AppAcceptance; storage: StorageAcceptance }) {
const { AppLimits, AssetLimits, DefaultWindowRadius, ShapeLimits, ThemeLimit, ThemeFileLimit, ThemeTokenRule } = accepted.app.limits;
const { StorageLimits, AttachmentLimits } = accepted.storage;
return `// Generated by bun run schema:generate. Do not edit.
use serde::{Serialize, Deserialize};
use serde_json::Value;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code { ${CoreErrorCodes.map(pascal).join(", ")} }
impl Code {
    pub fn as_str(self) -> &'static str { match self {
${CoreErrorCodes.map(code => `        Self::${pascal(code)} => "${code}",`).join("\n")}
    } }
}
impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}
/// The platform level this build runs; a template or document above it needs a newer app.
pub const PACKAGE_FORMAT: u64 = ${PackageFormat};
pub const RUNTIME_ABI: u64 = ${RuntimeABI};
#[cfg(feature = "storage")]
pub const HELPER_PROTOCOL: u64 = ${HelperProtocol.version};
/// The CSS \`border-radius\` of a window whose manifest names no shape.
pub(crate) const DEFAULT_WINDOW_RADIUS: &str = "${DefaultWindowRadius}";
/// Effective theme JSON, and a theme file, in UTF-8 bytes.
pub(crate) const THEME_LIMIT: usize = ${ThemeLimit};
pub(crate) const THEME_FILE_LIMIT: usize = ${ThemeFileLimit};
/// The longest theme token name, the most tokens a palette declares, and the token
/// prefix the host reserves.
pub(crate) const THEME_NAME_LIMIT: usize = ${ThemeTokenRule.nameLength};
pub(crate) const THEME_TOKENS: usize = ${ThemeTokenRule.tokens};
pub(crate) const THEME_RESERVED_PREFIX: &str = "${ThemeTokenRule.reservedPrefix}";
/// The lowercase Crockford alphabet of minted and derived row IDs.
pub(crate) const ID_ALPHABET: &[u8] = b"${RowIdRule.mintAlphabet}";
/// A document's saved checkpoint plus updates, in bytes, and its update rows.
pub const STORAGE_BYTES: usize = ${StorageLimits.bytes};
pub const STORAGE_ROWS: usize = ${StorageLimits.rows};
/// An app: its manifest and its descriptor or initial values, in bytes, and its longest
/// asset path; one asset's bytes, the asset count and their total bytes; and the largest
/// image it may carry, per side and in pixels.
#[cfg(feature = "storage")]
pub(crate) const MANIFEST_BYTES: usize = ${AppLimits.manifest};
pub(crate) const APP_TEXT_BYTES: usize = ${AppLimits.text};
#[cfg(feature = "storage")]
pub(crate) const ASSET_PATH_BYTES: usize = ${AppLimits.assetPath};
pub const ASSET_FILE_BYTES: usize = ${AssetLimits.file};
pub const ASSET_COUNT: usize = ${AssetLimits.count};
pub const ASSET_BYTES: usize = ${AssetLimits.bytes};
pub const IMAGE_SIDE: usize = ${AssetLimits.imageSide};
pub const IMAGE_PIXELS: usize = ${AssetLimits.imagePixels};
/// A document's attachments: one file's bytes, their total bytes and their count.
pub const ATTACHMENT_FILE_BYTES: usize = ${AttachmentLimits.file};
pub const ATTACHMENT_BYTES: usize = ${AttachmentLimits.total};
pub const ATTACHMENT_COUNT: usize = ${AttachmentLimits.count};
/// A window shape's longest path data and radius list, and its largest view box side.
pub(crate) const SHAPE_PATH: usize = ${ShapeLimits.path};
pub(crate) const SHAPE_RADIUS: usize = ${ShapeLimits.radius};
pub(crate) const SHAPE_VIEW_BOX: f64 = ${ShapeLimits.viewBox}.0;
/// A batch's most intents and an intent's longest path; a page request's largest payload.
pub(crate) const BATCH_INTENTS: usize = ${BatchLimits.intents};
pub(crate) const PATH_SEGMENTS: usize = ${BatchLimits.pathSegments};
pub(crate) const PAGE_PAYLOAD: usize = ${PagePayloadLimit};
/// The largest socket request, an attachment upload; no envelope the core checks is larger.
#[cfg(feature = "storage")]
pub const SOCKET_ATTACHMENT: usize = ${SocketLimits.attachment};
#[cfg(feature = "storage")]
pub const SOCKET_REQUEST: usize = ${SocketLimits.request};
/// An attachment's identity: the SHA-256 of its bytes, in lowercase hex.
#[cfg(feature = "storage")]
pub(crate) fn valid_attachment_id(id: &str) -> bool {
    id.len() == ${AttachmentIdRule.length} && id.bytes().all(|b| b"${AttachmentIdRule.characters}".contains(&b))
}
pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= ${RowIdRule.maximum} && value.bytes().all(|b| b"${RowIdRule.characters}".contains(&b))
}
${union("Segment", Segment, "Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize")}
${union("Anchor", Anchor, "Debug, Deserialize")}
${union("Hunk", TextHunk, "Clone, Debug, PartialEq, Serialize")}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PatchOp {
${PatchOp.anyOf.map(schema => {
  const { type, ...fields } = schema.properties;
  return `    ${upper(type.const)} { ${Object.entries(fields).map(([key, value]) => `${key}: ${rust(value)}`).join(", ")} },`;
}).join("\n")}
}
impl PatchOp {
    pub fn path(&self) -> &[Segment] { match self {
${PatchOp.anyOf.map(schema => `        Self::${upper(schema.properties.type.const)} { path, .. } => path,`).join("\n")}
    } }
}
${record("Publication", OwnerPublicationSchema, { previous: "u64", sequence: "u64", ops: "Vec<PatchOp>", theme: "std::collections::BTreeMap<String, String>" })}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Intent {
${Object.entries(variants)
  .map(
    ([kind, fields]) =>
      `    ${upper(kind)} { ${Object.entries(fields)
        .map(([key, schema]) => `${key}: ${rust(schema)}`)
        .join(", ")} },`,
  )
  .join("\n")}
}
impl Intent {
    /// The data path; palette intents have none.
    pub fn path(&self) -> &[Segment] { match self {
${Object.entries(variants)
  .map(([kind, fields]) => "path" in fields ? `        Self::${upper(kind)} { path, .. } => path,` : `        Self::${upper(kind)} { .. } => &[],`)
  .join("\n")}
    } }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub struct Batch { ${Object.entries(BatchSchema.properties)
  .map(([key, schema]) => `pub ${key}: ${key === "intents" ? "Vec<Intent>" : rust(schema)}`)
  .join(", ")} }
/// A text selection in UTF-16 offsets.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection { ${Object.entries(Selection.properties)
  .map(([key, schema]) => `pub ${key}: ${rust(schema)}`)
  .join(", ")} }
/// A shared theme file: the template it was made for and its palette.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFile { ${Object.entries(ThemeFileSchema.properties)
  .map(([key, schema]) => `pub ${key}: ${key === "values" ? "std::collections::BTreeMap<String, String>" : rust(schema)}`)
  .join(", ")} }
/// A built app (a build's \`app.json\`), as the file engine packs it into the \`app\` row.
/// Each part stays the JSON text the build wrote: its own validator reads it, in order.
#[cfg(feature = "storage")]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub struct AppRow { ${Object.entries(AppRowSchema.properties)
  .map(([key, schema]) => `pub ${key}: ${(schema as any).type === "integer" ? "u64" : "Box<serde_json::value::RawValue>"}`)
  .join(", ")} }

${socketEnum("EngineRequest", requests(EngineRequestSchema), "Debug, Serialize", true)}
#[cfg(feature = "storage")]
impl EngineRequest {
    /// Parse each member directly from JSON so opaque payloads retain their bytes.
    pub fn parse(input: &str) -> serde_json::Result<Self> {
        #[derive(Deserialize)] struct Header { method: String }
        let header: Header = serde_json::from_str(input)?;
        match header.method.as_str() {
${requests(EngineRequestSchema).map(({ method, fields }) => `            "${method}" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                #[allow(non_snake_case, dead_code)]
                struct Body { method: String, ${fields.map(([key, schema]) => `${key}: ${engineRust(schema)}`).join(", ")} }
                let ${fields.length ? "body" : "_body"}: Body = serde_json::from_str(input)?;
                Ok(Self::${socketName(method)} { ${fields.map(([key]) => `${key}: body.${key}`).join(", ")} })
            },`).join("\n")}
            _ => Err(<serde_json::Error as serde::de::Error>::custom("Unknown engine method")),
        }
    }
    pub fn method(&self) -> &'static str { match self {
${requests(EngineRequestSchema).map(({ method }) => `        Self::${socketName(method)} { .. } => "${method}",`).join("\n")}
    } }
}
${socketEnum("EngineSuccess", requests(EngineSuccessSchema), "Debug, Serialize", true)}
${socketEnum("SocketRequest", socketRequests, "Clone, Debug, Serialize, Deserialize")}
#[cfg(feature = "storage")]
impl SocketRequest {
    pub fn method(&self) -> &'static str { match self {
${socketRequests.map(({ method }) => `        Self::${socketName(method)} { .. } => "${method}",`).join("\n")}
    } }
    pub fn path(&self) -> &str { match self {
${socketRequests.map(({ method }) => `        Self::${socketName(method)} { documentPath, .. } => documentPath,`).join("\n")}
    } }
}
/// A page request. The core answers the document requests; the window's own (config,
/// readiness, resizing, errors) are the host's, so their fields go unread here.
${socketEnum("PageRequest", requests(PageRequestSchema), "Debug, Deserialize").trimStart().replace("#[allow(non_snake_case)]", "#[allow(non_snake_case, dead_code)]")}
${socketEnum("SocketSuccess", SocketSuccessSchema.anyOf.map((schema) => ({ method: schema.properties.method.const, fields: socketFields(schema) })), "Debug, Serialize")}
#[cfg(feature = "storage")]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OutcomeCode { ${OutcomeCodes.map(pascal).join(", ")} }
#[cfg(feature = "storage")]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub(crate) struct SocketFailure {
${socketFields(SocketFailureSchema).map(([key, schema]) => `    ${optional(schema as object) ? '#[serde(skip_serializing_if = "Option::is_none")] ' : ""}pub ${key}: ${key === "code" ? "OutcomeCode" : key === "opIndex" ? "Option<u32>" : socketRust(key, schema)},`).join("\n")}
}
`;
}
