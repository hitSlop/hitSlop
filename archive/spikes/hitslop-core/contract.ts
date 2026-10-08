import { Type, type Static } from "typebox";

// Experimental envelopes only. Production TypeBox contracts remain unchanged.
export const Segment = Type.Union([
  Type.String({ minLength: 1 }),
  Type.Object({ id: Type.String({ pattern: "^[0-9A-Za-z_-]{1,64}$" }) }, { additionalProperties: false }),
]);
const path = Type.Array(Segment, { minItems: 1, maxItems: 64 });
export const Anchor = Type.Union([
  Type.Object({ before: Type.String() }, { additionalProperties: false }),
  Type.Object({ after: Type.String() }, { additionalProperties: false }),
]);
export const variants = {
  set: { path, value: Type.Unknown() },
  insert: { path, value: Type.Unknown(), id: Type.Optional(Type.String()), at: Type.Optional(Anchor) },
  remove: { path, id: Type.String() },
  move: { path, id: Type.String(), at: Type.Optional(Anchor) },
  // First milestone rejects stale bases; ancestry/rebasing is an explicit later gate.
  splice: { path, base: Type.String(), index: Type.Integer({ minimum: 0 }), delete: Type.Integer({ minimum: 0 }), insert: Type.String() },
  // Counters: a nonzero safe-integer delta; the core also bounds the resulting sum.
  increment: { path, by: Type.Integer({ minimum: -9007199254740991, maximum: 9007199254740991 }) },
} as const;
export const Intent = Type.Union(Object.entries(variants).map(([type, fields]) =>
  Type.Object({ type: Type.Literal(type), ...fields }, { additionalProperties: false })));
export const Batch = Type.Object({ intents: Type.Array(Intent, { maxItems: 1000 }) }, { additionalProperties: false });
export type Batch = Static<typeof Batch>;


export const textFields = {
  session: Type.String(), draft: Type.String(), sequence: Type.Integer({minimum: 1}),
  parent: Type.Optional(Type.Integer({minimum: 1})), base: Type.String(), path,
  index: Type.Integer({minimum: 0}), delete: Type.Integer({minimum: 0}), insert: Type.String(),
  selectionStart: Type.Integer({minimum: 0}), selectionEnd: Type.Integer({minimum: 0}),
};
export const TextRequest = Type.Object(textFields, {additionalProperties: false});

// Command offsets explicitly refer to state when the host executor runs them.
// A renderer draft must use TextRequest instead; it must never silently rebase.
const {base: _base, ...currentSplice} = variants.splice;
export const CurrentCommand = Type.Object({
  intents: Type.Array(Type.Union(Object.entries({...variants,splice:currentSplice}).map(([type,fields]) =>
    Type.Object({type:Type.Literal(type),...fields},{additionalProperties:false})
  )), {maxItems:1000})
},{additionalProperties:false});
export const PatchOp = Type.Union([
  Type.Object({type:Type.Literal('set'),path,value:Type.Unknown()},{additionalProperties:false}),
  Type.Object({type:Type.Literal('remove'),path},{additionalProperties:false}),
  Type.Object({type:Type.Literal('insertRow'),path,index:Type.Integer({minimum:0}),value:Type.Unknown()},{additionalProperties:false}),
  Type.Object({type:Type.Literal('deleteRow'),path,id:Type.String()},{additionalProperties:false}),
  Type.Object({type:Type.Literal('moveRow'),path,id:Type.String(),index:Type.Integer({minimum:0})},{additionalProperties:false}),
]);
