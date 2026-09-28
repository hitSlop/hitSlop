import { Type, type Static } from "typebox";

// Contract 4: shared native/WASM core envelopes. TypeBox is authoritative.
export const Segment = Type.Union([
  Type.String({ minLength: 1 }),
  Type.Object(
    { id: Type.String({ pattern: "^[0-9A-Za-z_-]{1,64}$" }) },
    { additionalProperties: false },
  ),
]);
const path = Type.Array(Segment, { minItems: 1, maxItems: 64 });
export const Anchor = Type.Union([
  Type.Object({ before: Type.String() }, { additionalProperties: false }),
  Type.Object({ after: Type.String() }, { additionalProperties: false }),
]);
export const variants = {
  set: { path, value: Type.Unknown() },
  insert: {
    path,
    value: Type.Unknown(),
    id: Type.Optional(Type.String()),
    at: Type.Optional(Anchor),
  },
  remove: { path, id: Type.String() },
  move: { path, id: Type.String(), at: Type.Optional(Anchor) },
  // CLI splices carry an execution-time base; renderer drafts use TextRequest.
  splice: {
    path,
    base: Type.String(),
    index: Type.Integer({ minimum: 0 }),
    delete: Type.Integer({ minimum: 0 }),
    insert: Type.String(),
  },
  // Counters: a nonzero safe-integer delta; the core also bounds the resulting sum.
  increment: { path, by: Type.Integer({ minimum: -9007199254740991, maximum: 9007199254740991 }) },
} as const;
export const Intent = Type.Union([
  Type.Object({ type: Type.Literal("set"), ...variants.set }, { additionalProperties: false }),
  Type.Object(
    { type: Type.Literal("insert"), ...variants.insert },
    { additionalProperties: false },
  ),
  Type.Object(
    { type: Type.Literal("remove"), ...variants.remove },
    { additionalProperties: false },
  ),
  Type.Object({ type: Type.Literal("move"), ...variants.move }, { additionalProperties: false }),
  Type.Object(
    { type: Type.Literal("splice"), ...variants.splice },
    { additionalProperties: false },
  ),
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

export const textFields = {
  session: Type.String(),
  draft: Type.String(),
  sequence: Type.Integer({ minimum: 1 }),
  parent: Type.Optional(Type.Integer({ minimum: 1 })),
  base: Type.String(),
  path,
  index: Type.Integer({ minimum: 0 }),
  delete: Type.Integer({ minimum: 0 }),
  insert: Type.String(),
  selectionStart: Type.Integer({ minimum: 0 }),
  selectionEnd: Type.Integer({ minimum: 0 }),
};
export const TextRequest = Type.Object(textFields, { additionalProperties: false });

// Command offsets explicitly refer to state when the host executor runs them.
// A renderer draft must use TextRequest instead; it must never silently rebase.
const { base: _base, ...currentSplice } = variants.splice;
export const CurrentIntent = Type.Union(
  Object.entries({ ...variants, splice: currentSplice }).map(([type, fields]) =>
    Type.Object({ type: Type.Literal(type), ...fields }, { additionalProperties: false }),
  ),
);
export const CurrentCommand = Type.Object(
  {
    intents: Type.Array(CurrentIntent, { maxItems: 1000 }),
  },
  { additionalProperties: false },
);
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
    session: identity,
    sequence,
    version: Type.String(),
    value: Type.Unknown(),
    issues: Type.Array(OwnerIssueSchema),
  },
  { additionalProperties: false },
);
export const OwnerPatchSchema = Type.Object(
  {
    session: identity,
    previous: sequence,
    sequence,
    ops: Type.Array(PatchOp),
    issues: Type.Array(OwnerIssueSchema),
  },
  { additionalProperties: false },
);
export const OwnerPublicationSchema = Type.Object(
  {
    version: Type.String(),
    ids: Type.Array(Type.String()),
    patch: OwnerPatchSchema,
    patchBuildMS: Type.Optional(Type.Union([Type.Number(), Type.Null()])),
    text: Type.Optional(
      Type.Object(
        {
          draft: identity,
          sequence,
          authored: Type.String(),
          selectionStart: sequence,
          selectionEnd: sequence,
        },
        { additionalProperties: false },
      ),
    ),
  },
  { additionalProperties: false },
);
export const OwnerStatusSchema = Type.Enum(["pending", "saved", "save-failed"]);
export const OwnerRequestSchema = Type.Union([
  Type.Object(
    { id: identity, session: identity, method: Type.Literal("apply"), batch: Batch },
    { additionalProperties: false },
  ),
  Type.Object(
    { id: identity, session: identity, method: Type.Literal("text"), request: TextRequest },
    { additionalProperties: false },
  ),
  Type.Object(
    { id: identity, session: identity, method: Type.Literal("releaseDraft"), draft: identity },
    { additionalProperties: false },
  ),
  Type.Object(
    { id: identity, method: Type.Enum(["state", "flush"]) },
    { additionalProperties: false },
  ),
]);
export const OwnerReplySchema = Type.Union([
  Type.Object(
    {
      id: identity,
      ok: Type.Literal(true),
      state: Type.Optional(OwnerStateSchema),
      publication: Type.Optional(OwnerPublicationSchema),
      status: OwnerStatusSchema,
    },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      id: identity,
      ok: Type.Literal(false),
      code: Type.Enum([
        "rejected",
        "session_changed",
        "closing",
        "unsupported_operation",
        "unknown_outcome",
        "save_failed",
        "owner_invalidated",
      ]),
      error: Type.String(),
    },
    { additionalProperties: false },
  ),
]);
export type OwnerState = Static<typeof OwnerStateSchema>;
export type OwnerPublication = Static<typeof OwnerPublicationSchema>;
export type OwnerStatus = Static<typeof OwnerStatusSchema>;
export type OwnerRequest = Static<typeof OwnerRequestSchema>;
export type OwnerReply = Static<typeof OwnerReplySchema>;
export type OwnerIntent = Static<typeof Intent>;
export type OwnerPath = Static<typeof path>;
export type OwnerTextRequest = Static<typeof TextRequest>;
export type OwnerPatchOp = Static<typeof PatchOp>;

const socketBase = { id: identity, documentPath: Type.String({ minLength: 1, maxLength: 4096 }) };
const socketMutation = { ...socketBase, epoch: identity };
export const OwnerSocketRequestSchema = Type.Union([
  Type.Object(
    { ...socketBase, method: Type.Enum(["hello", "get", "schema", "snapshot"]) },
    { additionalProperties: false },
  ),
  Type.Object(
    { ...socketMutation, method: Type.Literal("apply"), op: CurrentIntent },
    { additionalProperties: false },
  ),
  Type.Object(
    {
      ...socketMutation,
      method: Type.Literal("batch"),
      ops: Type.Array(CurrentIntent, { maxItems: 1000 }),
    },
    { additionalProperties: false },
  ),
]);

export const OwnerContractsSchema = Type.Object(
  {
    batch: Batch,
    text: TextRequest,
    command: CurrentCommand,
    state: OwnerStateSchema,
    publication: OwnerPublicationSchema,
    status: OwnerStatusSchema,
    request: OwnerRequestSchema,
    reply: OwnerReplySchema,
    socketRequest: OwnerSocketRequestSchema,
  },
  { additionalProperties: false },
);
