import { Strict } from "./strict";
import { Type, type Static } from "typebox";
import { BatchLimits, CoreErrorCodes, RowIdRule, ThemeFileLimit } from "./constants";
import { ThemeChangesSchema, ThemeValuesSchema } from "./values";

// Document core payloads. TypeBox is authoritative; Rust wire types are generated.
// Field names and record keys are strings; rows are `{id}`; scalar-list elements `{index}`.
const RowIdSchema = Type.String({
  pattern: `^[${RowIdRule.characters}]{1,${RowIdRule.maximum}}$`,
});
export const SegmentSchema = Type.Union([
  Type.String({ minLength: 1 }),
  Strict({ id: RowIdSchema }),
  Strict({ index: Type.Integer({ minimum: 0 }) }),
]);
const path = Type.Array(SegmentSchema, { minItems: 1, maxItems: BatchLimits.pathSegments });
export const AnchorSchema = Type.Union([
  Strict({ before: Type.String() }),
  Strict({ after: Type.String() }),
]);
export const variants = {
  // Scalars, optional objects, and whole text fields (as the text is at execution).
  set: { path, value: Type.Unknown() },
  // Rows take `id`/`at`; scalar-list elements take `index` (default: append).
  insert: {
    path,
    value: Type.Unknown(),
    id: Type.Optional(Type.String()),
    at: Type.Optional(AnchorSchema),
    index: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  // Rows by `id`; scalar-list elements by `index` and `count` (default 1).
  remove: {
    path,
    id: Type.Optional(Type.String()),
    index: Type.Optional(Type.Integer({ minimum: 0 })),
    count: Type.Optional(Type.Integer({ minimum: 1 })),
  },
  move: { path, id: Type.String(), at: Type.Optional(AnchorSchema) },
  // Removes an optional field's value; a no-op when it is not set.
  clear: { path },
  // Counters: a nonzero safe-integer delta; the core also bounds the resulting sum.
  increment: {
    path,
    by: Type.Integer({ minimum: -Number.MAX_SAFE_INTEGER, maximum: Number.MAX_SAFE_INTEGER }),
  },
  // The value at `path` (the whole document when empty) becomes `value`. Only the
  // differences are written: rows are matched by `$id`, and rows and text keep their
  // identity, so open text fields keep their place.
  replace: { path: Type.Array(SegmentSchema, { maxItems: BatchLimits.pathSegments }), value: Type.Unknown() },
  // The palette, beside the data, from the window or an agent (never the page): each
  // listed color is set, or with `null` returned to the template's; `replace` returns
  // every unlisted color to the template's too.
  setTheme: { values: ThemeChangesSchema, replace: Type.Optional(Type.Boolean()) },
  // A theme file made for this document's template replaces the palette. Its text; only
  // the core parses it.
  importTheme: { file: Type.String({ minLength: 2, maxLength: ThemeFileLimit }) },
} as const;
/** The data intents, which the page sends. */
const OwnerIntentSchema = Type.Union([
  Strict({ type: Type.Literal("set"), ...variants.set }),
  Strict({ type: Type.Literal("insert"), ...variants.insert }),
  Strict({ type: Type.Literal("remove"), ...variants.remove }),
  Strict({ type: Type.Literal("move"), ...variants.move }),
  Strict({ type: Type.Literal("clear"), ...variants.clear }),
  Strict({ type: Type.Literal("increment"), ...variants.increment }),
  Strict({ type: Type.Literal("replace"), ...variants.replace }),
]);
/** The palette intents, which only the window and agents send; a batch may mix them with
 * data intents. */
const PaletteIntentSchema = Type.Union([
  Strict({ type: Type.Literal("setTheme"), ...variants.setTheme }),
  Strict({ type: Type.Literal("importTheme"), ...variants.importTheme }),
]);
const BatchSchema = Strict({ intents: Type.Array(OwnerIntentSchema, { maxItems: BatchLimits.intents }) });
/** A page's batch. */
export type Batch = Static<typeof BatchSchema>;

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
const EditTextSchema = Strict(editTextFields);

/** One hunk of a text change, in Unicode code points of the field's previous text. */
export const TextHunkSchema = Type.Union([
  Strict({ retain: Type.Integer({ minimum: 1 }) }),
  Strict({ insert: Type.String({ minLength: 1 }) }),
  Strict({ delete: Type.Integer({ minimum: 1 }) }),
]);
export const OwnerPatchOpSchema = Type.Union([
  Strict({ type: Type.Literal("set"), path, value: Type.Unknown() }),
  // An edit to an existing text field; the rest of the field is retained.
  Strict({ type: Type.Literal("text"), path, delta: Type.Array(TextHunkSchema, { minItems: 1 }) }),
  Strict({ type: Type.Literal("remove"), path }),
  Strict({
    type: Type.Literal("insertRow"),
    path,
    index: Type.Integer({ minimum: 0 }),
    value: Type.Unknown(),
  }),
  Strict({ type: Type.Literal("deleteRow"), path, id: Type.String() }),
  Strict({
    type: Type.Literal("moveRow"),
    path,
    id: Type.String(),
    index: Type.Integer({ minimum: 0 }),
  }),
]);

const sequence = Type.Integer({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER });
export const OwnerStateSchema = Strict({
  sequence,
  version: Type.String(),
  value: Type.Unknown(),
  theme: ThemeValuesSchema,
});
/** One accepted change. `previous` lets the page prove the stream is contiguous; `theme`,
 * the effective palette, is present only when it changed. */
export const OwnerPublicationSchema = Strict({
  previous: sequence,
  sequence,
  version: Type.String(),
  ops: Type.Array(OwnerPatchOpSchema),
  theme: Type.Optional(ThemeValuesSchema),
});

export const CoreErrorCodeSchema = Type.Enum(CoreErrorCodes, { title: "CoreErrorCode" });
export type CoreErrorCode = Static<typeof CoreErrorCodeSchema>;
export type Segment = Static<typeof SegmentSchema>;
/** Where a row goes: before or after the row with this `$id`. */
export type Anchor = Static<typeof AnchorSchema>;
export type OwnerState = Static<typeof OwnerStateSchema>;
export type OwnerPublication = Static<typeof OwnerPublicationSchema>;
export type OwnerIntent = Static<typeof OwnerIntentSchema>;
export type PaletteIntent = Static<typeof PaletteIntentSchema>;
export type OwnerPath = Segment[];
export type OwnerPatchOp = Static<typeof OwnerPatchOpSchema>;
export type EditText = Static<typeof EditTextSchema>;
export type TextHunk = Static<typeof TextHunkSchema>;
