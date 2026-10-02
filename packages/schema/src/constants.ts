// Plain values shared by the contracts, the page shell, the CLI, Rust and Swift
// (`bun run schema:generate`). TypeBox-free, so a consumer that needs only a limit or a
// code list never loads the schema library.

export const SlopCategories = [
  "productivity",
  "utilities",
  "finance",
  "media",
  "games",
  "developer-tools",
  "education",
  "business",
  "personal",
  "other",
] as const;
/** Manifest text fields: lengths in UTF-16 units, and patterns they must match. */
export const ManifestText = {
  slug: { minLength: 2, maxLength: 64, pattern: "^[a-z0-9]+(?:-[a-z0-9]+)*$" },
  title: { minLength: 1, maxLength: 80 },
  description: { minLength: 1, maxLength: 240 },
  authorName: { minLength: 1, maxLength: 80, pattern: "\\S" },
} as const;
/** Window size bounds, in points. */
export const WindowBounds = { minWidth: 240, minHeight: 180, max: 4096 } as const;
/** The CSS `border-radius` of a window whose manifest names no shape. */
export const DefaultWindowRadius = "22px";
/** Attachment sizes, and the longest file name and MIME type a reference keeps (UTF-8 bytes). */
export const AttachmentLimits = { file: 10 * 1024 * 1024, total: 100 * 1024 * 1024, count: 256, name: 255 } as const;
export const base64Length = (bytes: number) => 4 * Math.ceil(bytes / 3);
/** An attachment's ID: the SHA-256 of its bytes, in lowercase hex. */
export const AttachmentIdPattern = "^[a-f0-9]{64}$";
/** A theme is a palette: token names, their longest length, and colors as lowercase
 * `#rrggbb` or `#rrggbbaa` with one spelling per color (opaque colors omit `ff`). The
 * host reserves the `window-` prefix for window geometry. */
export const ThemeTokenRule = {
  name: "^[a-zA-Z][a-zA-Z0-9-]{0,63}$",
  nameLength: 64,
  value: "^#[0-9a-f]{6}(?:[0-9a-e][0-9a-f]|f[0-9a-e])?$",
  reservedPrefix: "window-",
  tokens: 256,
} as const;
/** A document's saved checkpoint plus updates: bytes, and update rows. */
export const StorageLimits = { bytes: 32 * 1024 * 1024, rows: 4096 } as const;
/** Effective theme JSON, in UTF-8 bytes. */
export const ThemeLimit = 64 * 1024;
/** A theme file: the largest effective theme plus its template and wrapper, in UTF-8
 * bytes, so every export can be imported again. */
export const ThemeFileLimit = ThemeLimit + 1024;
/** Diagnostic text a page reports to the host, in UTF-16 units. */
export const ErrorTextLimit = 4096;
/** The `Symbol.for` key on document errors (refusals and owner outcomes). The page's
 * unhandled-rejection hook reports a branded reason as an operation issue, not as an
 * application failure. */
export const OperationErrorBrand = "hitslop.operation-error";
/** The largest page request payload (a batch or text edit), in UTF-8 bytes. */
export const PagePayloadLimit = 4 * 1024 * 1024;
/** Pushes buffered for one page before a gap forces a fresh snapshot. */
export const PushLimits = { items: 256, bytes: 4 * 1024 * 1024 } as const;
/** Socket request sizes: every method, and attachment uploads. */
export const SocketLimits = { request: 1024 * 1024, attachment: 16 * 1024 * 1024 } as const;
/** Row `$id`s: the characters any ID may use, and the lowercase Crockford alphabet of
 * IDs the SDK and core mint. */
export const RowIdRule = {
  characters: "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_-",
  maximum: 64,
  mintAlphabet: "0123456789abcdefghjkmnpqrstvwxyz",
} as const;
export const IssueCodes = ["type_mismatch", "out_of_range", "unknown_field", "invalid_key", "invalid_id", "duplicate_id"] as const;
export const CoreErrorCodes = [
  "type_mismatch", "out_of_range", "path_not_found", "invalid_key", "exists", "duplicate_id",
  "invalid_request", "invalid_id", "invalid_path", "invalid_schema", "too_large", "stale_base",
  "invalid_version", "invalid_bytes", "missing_dependencies", "engine_error", "invalid_shape",
] as const;
export const PageErrorCodes = [
  "rejected",
  "owner_replaced",
  "closing",
  "save_failed",
  "owner_invalidated",
  "unknown_outcome",
] as const;
