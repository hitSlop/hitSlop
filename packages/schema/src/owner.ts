import { Type, type Static } from "typebox";
import { CoreErrorCodes, IssueCodes, PageErrorCodes, PagePayloadLimit, RowIdRule } from "./constants";
export { CoreErrorCodes, IssueCodes, PageErrorCodes, PagePayloadLimit, RowIdRule };

// Document core and page wire. TypeBox is authoritative; Rust and Swift are generated.
// Field names and record keys are strings; rows are `{id}`; scalar-list elements `{index}`.
export const RowId = Type.String({ pattern: `^[${RowIdRule.characters}]{1,${RowIdRule.maximum}}$` });
export const Segment = Type.Union([
  Type.String({ minLength: 1 }),
  Type.Object(
    { id: RowId },
    { additionalProperties: false },
  ),
  Type.Object({ index: Type.Integer({ minimum: 0 }) }, { additionalProperties: false }),
]);
const path = Type.Array(Segment, { minItems: 1, maxItems: 64 });
export const Anchor = Type.Union([
  Type.Object({ before: Type.String() }, { additionalProperties: false }),
  Type.Object({ after: Type.String() }, { additionalProperties: false }),
]);
export const variants = {
  // Scalars, optional objects, and whole text fields (as the text is at execution).
  set: { path, value: Type.Unknown() },
  // Rows take `id`/`at`; scalar-list elements take `index` (default: append).
  insert: {
    path,
    value: Type.Unknown(),
    id: Type.Optional(Type.String()),
    at: Type.Optional(Anchor),
    index: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  // Rows by `id`; scalar-list elements by `index` and `count` (default 1).
  remove: {
    path,
    id: Type.Optional(Type.String()),
    index: Type.Optional(Type.Integer({ minimum: 0 })),
    count: Type.Optional(Type.Integer({ minimum: 1 })),
  },
  move: { path, id: Type.String(), at: Type.Optional(Anchor) },
  // Removes an optional field's value; a no-op when it is not set.
  clear: { path },
  // Counters: a nonzero safe-integer delta; the core also bounds the resulting sum.
  increment: { path, by: Type.Integer({ minimum: -Number.MAX_SAFE_INTEGER, maximum: Number.MAX_SAFE_INTEGER }) },
} as const;
export const Intent = Type.Union([
  Type.Object({ type: Type.Literal("set"), ...variants.set }, { additionalProperties: false }),
  Type.Object({ type: Type.Literal("insert"), ...variants.insert }, { additionalProperties: false }),
  Type.Object({ type: Type.Literal("remove"), ...variants.remove }, { additionalProperties: false }),
  Type.Object({ type: Type.Literal("move"), ...variants.move }, { additionalProperties: false }),
  Type.Object({ type: Type.Literal("clear"), ...variants.clear }, { additionalProperties: false }),
  Type.Object(
    { type: Type.Literal("increment"), ...variants.increment },
    { additionalProperties: false },
  ),
]);
export const Batch = Type.Object(
  { intents: Type.Array(Intent, { maxItems: 1000 }) },
  { additionalProperties: false },
);
export type Batch = Static<typeof Batch>;

// Stateless text: the page's field was `from` at `base` (its last authored version) and
// is now `to`. The owner computes the edit script and merges it; no draft state.
export const editTextFields = {
  base: Type.String(),
  path,
  from: Type.String(),
  to: Type.String(),
  selectionStart: Type.Integer({ minimum: 0 }),
  selectionEnd: Type.Integer({ minimum: 0 }),
};
const EditTextRequest = Type.Object(editTextFields, { additionalProperties: false });

/** One hunk of a text change, in Unicode code points of the field's previous text. */
export const TextHunk = Type.Union([
  Type.Object({ retain: Type.Integer({ minimum: 1 }) }, { additionalProperties: false }),
  Type.Object({ insert: Type.String({ minLength: 1 }) }, { additionalProperties: false }),
  Type.Object({ delete: Type.Integer({ minimum: 1 }) }, { additionalProperties: false }),
]);
export const PatchOp = Type.Union([
  Type.Object(
    { type: Type.Literal("set"), path, value: Type.Unknown() },
    { additionalProperties: false },
  ),
  // An edit to an existing text field; the rest of the field is retained.
  Type.Object(
    { type: Type.Literal("text"), path, delta: Type.Array(TextHunk, { minItems: 1 }) },
    { additionalProperties: false },
  ),
  Type.Object({ type: Type.Literal("remove"), path }, { additionalProperties: false }),
  Type.Object(
    {
      type: Type.Literal("insertRow"),
      path,
      index: Type.Integer({ minimum: 0 }),
      value: Type.Unknown(),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    { type: Type.Literal("deleteRow"), path, id: Type.String() },
    { additionalProperties: false },
  ),
  Type.Object(
    { type: Type.Literal("moveRow"), path, id: Type.String(), index: Type.Integer({ minimum: 0 }) },
    { additionalProperties: false },
  ),
]);

const identity = Type.String({ minLength: 1, maxLength: 128 });
const sequence = Type.Integer({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER });
// Issues address stored anomalies. They do not authorize a repair on read. Rows are
// addressed by their effective `$id`, as in the snapshot; scalar-list elements and
// rows that are not objects by `{index}`.
const IssueCodeSchema = Type.Enum(IssueCodes);
export const OwnerIssueSchema = Type.Object(
  {
    code: IssueCodeSchema,
    path: Type.Array(Segment, { maxItems: 64 }),
  },
  { additionalProperties: false },
);
export const OwnerStateSchema = Type.Object(
  {
    sequence,
    version: Type.String(),
    value: Type.Unknown(),
    issues: Type.Array(OwnerIssueSchema),
  },
  { additionalProperties: false },
);
/** One accepted change. `previous` lets the page prove the stream is contiguous;
 * `issues`, the complete current list, is present only when it changed. */
export const OwnerPublicationSchema = Type.Object(
  {
    previous: sequence,
    sequence,
    version: Type.String(),
    ops: Type.Array(PatchOp),
    issues: Type.Optional(Type.Array(OwnerIssueSchema)),
  },
  { additionalProperties: false },
);

// Page → host. `view` names the attached page; a request from a replaced page, or one
// queued before discard, is refused with `owner_replaced` and never applied. Document
// payloads cross as JSON text that only the core parses: a `Batch` or an `EditText`.
const pageBase = { id: identity, view: identity };
const payload = Type.String({ minLength: 2, maxLength: PagePayloadLimit });
const PageRequestSchema = Type.Union([
  Type.Object({ ...pageBase, method: Type.Literal("open") }, { additionalProperties: false }),
  Type.Object(
    { ...pageBase, method: Type.Literal("apply"), batch: payload },
    { additionalProperties: false },
  ),
  Type.Object(
    { ...pageBase, method: Type.Literal("text"), request: payload },
    { additionalProperties: false },
  ),
  Type.Object({ ...pageBase, method: Type.Literal("flush") }, { additionalProperties: false }),
]);
export const CoreErrorCodeSchema = Type.Enum(CoreErrorCodes);
export type CoreErrorCode = Static<typeof CoreErrorCodeSchema>;

const PageErrorCodeSchema = Type.Enum(PageErrorCodes);
const ReplyFailure = Type.Object(
  { id: identity, ok: Type.Literal(false), code: PageErrorCodeSchema, error: Type.String(),
    reason: Type.Optional(CoreErrorCodeSchema), opIndex: Type.Optional(sequence) },
  { additionalProperties: false },
);
const success = { id: identity, ok: Type.Literal(true) };
// `state` is the core's snapshot JSON (an `OwnerState`), passed through unparsed.
export const OpenReplySchema = Type.Union([
  Type.Object({ ...success, state: Type.String() }, { additionalProperties: false }),
  ReplyFailure,
]);
export const ApplyReplySchema = Type.Union([
  Type.Object({ ...success, sequence, ids: Type.Array(Type.String()) }, { additionalProperties: false }),
  ReplyFailure,
]);
export const TextReplySchema = Type.Union([
  Type.Object({ ...success, sequence, authored: Type.String(), selectionStart: sequence, selectionEnd: sequence },
    { additionalProperties: false }),
  ReplyFailure,
]);
export const FlushReplySchema = Type.Union([
  Type.Object(success, { additionalProperties: false }), ReplyFailure,
]);
const PageReplySchema = Type.Union([OpenReplySchema, ApplyReplySchema, TextReplySchema, FlushReplySchema]);
/** Host → page, fenced to the view that was current when delivery was enqueued. */
export const PagePushSchema = Type.Union([
  Type.Object({ view: identity, type: Type.Literal("publication"), publication: OwnerPublicationSchema },
    { additionalProperties: false }),
  Type.Object({ view: identity, type: Type.Literal("resync") }, { additionalProperties: false }),
]);

export type OwnerState = Static<typeof OwnerStateSchema>;
export type OwnerPublication = Static<typeof OwnerPublicationSchema>;
export type OwnerIntent = Static<typeof Intent>;
export type OwnerPath = Static<typeof path>;
export type OwnerPatchOp = Static<typeof PatchOp>;
export type EditText = Static<typeof EditTextRequest>;
export type PageRequest = Static<typeof PageRequestSchema>;
export type PageErrorCode = Static<typeof PageErrorCodeSchema>;
export type PagePush = Static<typeof PagePushSchema>;

export const OwnerContractsSchema = Type.Object(
  {
    batch: Batch,
    editText: EditTextRequest,
    state: OwnerStateSchema,
    publication: OwnerPublicationSchema,
    request: PageRequestSchema,
    reply: PageReplySchema,
    push: PagePushSchema,
  },
  { additionalProperties: false },
);
