import { Type, type Static } from "typebox";
const path = Type.Array(
  Type.Union([
    Type.String(),
    Type.Object({ id: Type.String() }),
    Type.Object({ key: Type.String() }),
    Type.Object({ index: Type.Integer({ minimum: 0 }) }),
  ]),
  { maxItems: 64 },
);
const value = Type.Unknown(); // Native boundary uses recursive JSONValue, never Any.
export const variants = {
  set: { path, value },
  clear: { path },
  assign: { path, value },
  splice: {
    path,
    index: Type.Integer({ minimum: 0 }),
    deleteCount: Type.Integer({ minimum: 0 }),
    text: Type.String(),
  },
  mark: {
    path,
    start: Type.Integer({ minimum: 0 }),
    end: Type.Integer({ minimum: 0 }),
    key: Type.String(),
    value,
  },
  insert: { path, index: Type.Integer({ minimum: 0 }), id: Type.String(), value },
  remove: { path, index: Type.Integer({ minimum: 0 }) },
  move: { path, from: Type.Integer({ minimum: 0 }), to: Type.Integer({ minimum: 0 }) },
  increment: { path, amount: Type.Number() },
} as const;
export const OperationSchema = Type.Union(
  Object.entries(variants).map(([type, fields]) =>
    Type.Object({ type: Type.Literal(type), ...fields }, { additionalProperties: false }),
  ),
);
// Preserve the discriminant in TS instead of widening Object.entries' result.
export type Operation = {
  [K in keyof typeof variants]: { type: K } & Static<
    ReturnType<typeof Type.Object<(typeof variants)[K]>>
  >;
}[keyof typeof variants];
export type Path = Static<typeof path>;
