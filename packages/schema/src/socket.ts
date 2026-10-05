import { Strict } from "./strict";
import * as T from "typebox";
import {
  ThemeTokenSchema,
  ThemeValuesSchema,
  ThemeStateSchema,
  AttachmentIDSchema,
  AttachmentBytesSchema,
  AttachmentInfoSchema,
  OutcomeCodeSchema,
} from "./values";
import { CoreErrorCodeSchema, OwnerStateSchema } from "./core";
import { ExportFormats, SocketLimits, ThemeFileLimit } from "./constants";

const identity = T.String({ minLength: 1, maxLength: 128 });
const path = T.String({ minLength: 1, maxLength: 4096 });
const base = {
  documentPath: path,
};
const mutation = { ...base, epoch: identity };
// Operations are an array of intents encoded as JSON text that only the core parses.
const operations = T.String({ minLength: 2, maxLength: SocketLimits.request });
export const SocketRequestSchema = T.Union([
  Strict({ ...base, method: T.Literal("attachments.list") }),
  Strict({ ...base, method: T.Literal("attachments.read"), attachmentID: AttachmentIDSchema }),
  Strict({ ...mutation, method: T.Literal("attachments.put"), bytes: AttachmentBytesSchema }),
  Strict({ ...base, method: T.Literal("theme.get") }),
  Strict({ ...mutation, method: T.Literal("theme.set"), values: ThemeValuesSchema }),
  Strict({ ...mutation, method: T.Literal("theme.reset"), token: T.Optional(ThemeTokenSchema) }),
  Strict({ ...base, method: T.Literal("theme.export") }),
  // The file's text; only the core parses it.
  Strict({ ...mutation, method: T.Literal("theme.import"), file: T.String({ minLength: 2, maxLength: ThemeFileLimit }) }),
  Strict({ ...base, method: T.Enum(["hello", "get"]) }),
  Strict({ ...mutation, method: T.Literal("batch"), ops: operations }),
  Strict({
    ...mutation,
    method: T.Literal("export"),
    format: T.Enum(ExportFormats),
    output: path,
  }),
]);
/** Every successful result names its method and carries all fields that method promises. */
export const SocketSuccessSchema = T.Union([
  Strict({ ok: T.Literal(true), method: T.Literal("hello"), epoch: identity, coreBuildId: identity }),
  Strict({ ok: T.Literal(true), method: T.Literal("get"), epoch: identity,
    state: Strict({ schema: T.Object({}, { additionalProperties: true }), state: OwnerStateSchema }) }),
  Strict({ ok: T.Literal(true), method: T.Literal("batch"), epoch: identity,
    ids: T.Array(T.String()), sequence: T.Integer({ minimum: 0 }) }),
  // A closed export reads a snapshot without creating an owner or epoch.
  Strict({ ok: T.Literal(true), method: T.Literal("export"), epoch: T.Optional(identity), output: path }),
  Strict({ ok: T.Literal(true), method: T.Literal("theme.get"), epoch: identity, state: ThemeStateSchema }),
  Strict({ ok: T.Literal(true), method: T.Literal("theme.set"), epoch: identity, state: ThemeStateSchema }),
  Strict({ ok: T.Literal(true), method: T.Literal("theme.reset"), epoch: identity, state: ThemeStateSchema }),
  Strict({ ok: T.Literal(true), method: T.Literal("theme.import"), epoch: identity, state: ThemeStateSchema }),
  Strict({ ok: T.Literal(true), method: T.Literal("theme.export"), epoch: identity, state: Strict({ file: T.String() }) }),
  Strict({ ok: T.Literal(true), method: T.Literal("attachments.list"), epoch: identity, state: T.Array(AttachmentInfoSchema) }),
  Strict({ ok: T.Literal(true), method: T.Literal("attachments.read"), epoch: identity, state: Strict({ bytes: AttachmentBytesSchema }) }),
  Strict({ ok: T.Literal(true), method: T.Literal("attachments.put"), epoch: identity, state: AttachmentInfoSchema }),
]);
/** A classified failure. Unknown outcomes are explicit, never inferred from missing fields. */
export const SocketFailureSchema = Strict({
  ok: T.Literal(false),
  epoch: T.Optional(identity),
  error: T.String(),
  code: OutcomeCodeSchema,
  reason: T.Optional(CoreErrorCodeSchema),
  opIndex: T.Optional(T.Integer({ minimum: 0 })),
});
export const SocketReplySchema = T.Union([...SocketSuccessSchema.anyOf, SocketFailureSchema]);
/** Methods whose requests carry the owner's epoch: a failure leaves an outcome to report. */
export const EpochMethods: ReadonlySet<SocketMethod> = new Set(
  SocketRequestSchema.anyOf.flatMap((member) =>
    "epoch" in member.properties && "const" in member.properties.method ? [member.properties.method.const as SocketMethod] : [],
  ),
);
/** A live owner's discovery, in the registry (`~/.hitslop/live`): where it listens. Clients
 * learn the epoch from `hello`. */
export const SocketDiscoverySchema = Strict({
  socket: path,
  documentPath: path,
});

export type SocketRequest = T.Static<typeof SocketRequestSchema>;
export type SocketMethod = SocketRequest["method"];
/** A request as a client hands it to the helper, which adds the owner's epoch. */
export type HelperRequest = SocketRequest extends infer R ? (R extends unknown ? Omit<R, "epoch"> : never) : never;
/** The helper request for `M`, whose method is exactly `M`. */
export type HelperRequestFor<M extends SocketMethod> = HelperRequest extends infer R
  ? R extends { method: infer K }
    ? M extends K
      ? Omit<R, "method"> & { method: M }
      : never
    : never
  : never;
export type SocketReply = T.Static<typeof SocketReplySchema>;

export type SocketSuccess = T.Static<typeof SocketSuccessSchema>;
export type SocketFailure = T.Static<typeof SocketFailureSchema>;
export type SocketSuccessFor<M extends SocketMethod> = Extract<SocketSuccess, { method: M }>;
export type SocketReplyFor<M extends SocketMethod> = SocketSuccessFor<M> | SocketFailure;
