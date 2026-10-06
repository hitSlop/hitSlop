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
  "health",
  "creative",
  "music",
  "other",
] as const;
/** Manifest text fields: lengths in code points, and patterns they must match. */
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
export const AttachmentIdRule = { length: 64, characters: "0123456789abcdef" } as const;
export const AttachmentIdPattern = `^[${AttachmentIdRule.characters}]{${AttachmentIdRule.length}}$`;
/** A theme is a palette of at most `tokens` colors: token names up to `nameLength`
 * characters, and colors as lowercase `#rrggbb` or `#rrggbbaa` with one spelling per color
 * (opaque colors omit `ff`). The host reserves the `window-` prefix for window geometry.
 * The core owns these rules (`crates/hitslop-core/src/theme.rs`). */
export const ThemeTokenRule = { nameLength: 64, reservedPrefix: "window-", tokens: 256 } as const;
/** An app's assets: one asset's bytes, the asset count and their total bytes. Images an
 * app carries or a capture emits are at most `imageSide` pixels on a side and
 * `imagePixels` in all. The core checks them when it packs and when it opens a file. */
export const AssetLimits = {
  file: 25 * 1024 * 1024,
  count: 256,
  bytes: 50 * 1024 * 1024,
  imageSide: 16_384,
  imagePixels: 24_000_000,
} as const;
/** An app's stored JSON in UTF-8 bytes: its manifest, and its descriptor or initial
 * values; and the longest asset path. The core checks them when it packs and opens. */
export const AppLimits = { manifest: 64 * 1024, text: 4 * 1024 * 1024, assetPath: 240 } as const;
/** Window shapes: the longest path data and radius list, and the largest view box side.
 * The core owns the shape grammar (`crates/hitslop-core/src/shape.rs`). */
export const ShapeLimits = { path: 4096, radius: 256, viewBox: 16_384 } as const;
/** What `slop export` writes. */
export const ExportFormats = ["png", "pdf"] as const;
/** A batch: the most intents it carries, and the longest path an intent names. */
export const BatchLimits = { intents: 1000, pathSegments: 64 } as const;
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
/** Codes may grow; apps treat an unfamiliar one as a refusal they cannot name. */
export const CoreErrorCodes = [
  "type_mismatch", "out_of_range", "path_not_found", "invalid_key", "exists", "duplicate_id",
  "invalid_request", "invalid_id", "invalid_path", "invalid_schema", "too_large", "stale_base",
  "invalid_version", "invalid_bytes", "missing_dependencies", "engine_error", "invalid_shape",
  "requires_update", "is_template",
] as const;
/** Persisted package syntax (manifest, resources and descriptor encoding). */
export const PackageFormat = 1;
/** App-facing ctx behavior. Independent of package syntax and storage layout. */
export const RuntimeABI = 1;
/** The native helper's command line (`hitslop-native`): the version a CLI speaks, and the
 * oldest one a helper still serves. App updates keep serving every version in the range,
 * so a CLI keeps working until the minimum passes it. */
export const HelperProtocol = { version: 1, minimum: 1 } as const;
/** What a failed page or socket request means. `rejected`, `owner_replaced`, `closing` and
 * `owner_invalidated` were not applied; `save_failed` was applied but is not yet durable;
 * `unknown_outcome` needs a read before relying on it. Codes may grow. */
export const OutcomeCodes = [
  "rejected",
  "owner_replaced",
  "closing",
  "save_failed",
  "owner_invalidated",
  "unknown_outcome",
] as const;
