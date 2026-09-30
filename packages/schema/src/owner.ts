import { Type, type Static } from "typebox";

// Document core and page wire. TypeBox is authoritative; Rust and Swift are generated.
// Field names and record keys are strings; rows are `{id}`; scalar-list elements `{index}`.
export const Segment = Type.Union([
  Type.String({ minLength: 1 }),
  Type.Object(
    { id: Type.String({ pattern: "^[0-9A-Za-z_-]{1,64}$" }) },
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
  increment: { path, by: Type.Integer({ minimum: -9007199254740991, maximum: 9007199254740991 }) },
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
export const EditTextRequest = Type.Object(editTextFields, { additionalProperties: false });

export const PatchOp = Type.Union([
  Type.Object(
    { type: Type.Literal("set"), path, value: Type.Unknown() },
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
// Issues address stored anomalies. They do not authorize a repair on read.
export const OwnerIssueSchema = Type.Object(
  {
    code: Type.String({ minLength: 1 }),
    path: Type.Array(Type.Unknown()),
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
/** One accepted change. `previous` lets the page prove the stream is contiguous. */
export const OwnerPublicationSchema = Type.Object(
  {
    previous: sequence,
    sequence,
    version: Type.String(),
    ops: Type.Array(PatchOp),
    issues: Type.Array(OwnerIssueSchema),
  },
  { additionalProperties: false },
);

// Page → host. `view` names the attached page; a request from a replaced page, or one
// queued before discard, is refused with `owner_replaced` and never applied.
const pageBase = { id: identity, view: identity };
export const PageRequestSchema = Type.Union([
  Type.Object({ ...pageBase, method: Type.Literal("open") }, { additionalProperties: false }),
  Type.Object(
    { ...pageBase, method: Type.Literal("apply"), batch: Batch },
    { additionalProperties: false },
  ),
  Type.Object(
    { ...pageBase, method: Type.Literal("text"), request: EditTextRequest },
    { additionalProperties: false },
  ),
  Type.Object({ ...pageBase, method: Type.Literal("flush") }, { additionalProperties: false }),
]);
export const PageErrorCodeSchema = Type.Enum([
  "rejected",
  "owner_replaced",
  "closing",
  "save_failed",
  "owner_invalidated",
  "unknown_outcome",
]);
export const PageReplySchema = Type.Union([
  Type.Object(
    {
      id: identity,
      ok: Type.Literal(true),
      // open: the snapshot the push stream continues from, and save status.
      state: Type.Optional(OwnerStateSchema),
      savedSequence: Type.Optional(sequence),
      saveFailure: Type.Optional(Type.Union([Type.String(), Type.Null()])),
      // apply and text: the publication sequence that carries the change.
      sequence: Type.Optional(sequence),
      ids: Type.Optional(Type.Array(Type.String())),
      // text: the version right after this edit on its own branch, and the caret.
      authored: Type.Optional(Type.String()),
      selectionStart: Type.Optional(sequence),
      selectionEnd: Type.Optional(sequence),
    },
    { additionalProperties: false },
  ),
  Type.Object(
    { id: identity, ok: Type.Literal(false), code: PageErrorCodeSchema, error: Type.String() },
    { additionalProperties: false },
  ),
]);
/** Host → page, in order, through `__hitslop.publish(pushes)`. */
export const PagePushSchema = Type.Union([
  Type.Object(
    { type: Type.Literal("publication"), publication: OwnerPublicationSchema },
    { additionalProperties: false },
  ),
  Type.Object({ type: Type.Literal("saved"), sequence }, { additionalProperties: false }),
  Type.Object(
    { type: Type.Literal("failed"), error: Type.String() },
    { additionalProperties: false },
  ),
]);

export type OwnerState = Static<typeof OwnerStateSchema>;
export type OwnerPublication = Static<typeof OwnerPublicationSchema>;
export type OwnerIntent = Static<typeof Intent>;
export type OwnerPath = Static<typeof path>;
export type OwnerPatchOp = Static<typeof PatchOp>;
export type EditText = Static<typeof EditTextRequest>;
export type PageRequest = Static<typeof PageRequestSchema>;
export type PageReply = Static<typeof PageReplySchema>;
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
