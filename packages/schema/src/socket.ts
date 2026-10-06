import { Strict } from "./strict";
import * as T from "typebox";
import {
  ThemeValuesSchema,
  AttachmentIDSchema,
  AttachmentBytesSchema,
  AttachmentInfoSchema,
  OutcomeCodeSchema,
} from "./values";
import { CoreErrorCodeSchema } from "./core";
import { ExportFormats, SocketLimits } from "./constants";

const path = T.String({ minLength: 1, maxLength: 4096 });
// `protocol` is the command protocol the request is written in. The owner checks it
// before anything else, so an engine of another build gets a clear refusal.
const base = {
  protocol: T.Integer({ minimum: 1, maximum: Number.MAX_SAFE_INTEGER }),
  documentPath: path,
};
// Operations are an array of intents encoded as JSON text that only the core parses.
const operations = T.String({ minLength: 2, maxLength: SocketLimits.request });
export const SocketRequestSchema = T.Union([
  Strict({ ...base, method: T.Literal("attachments.list") }),
  Strict({ ...base, method: T.Literal("attachments.read"), attachmentID: AttachmentIDSchema }),
  Strict({ ...base, method: T.Literal("theme.export") }),
  Strict({ ...base, method: T.Literal("get") }),
  // `base`: the version the agent read the text at (`state.version` from `get`); its text
  // sets merge with edits made since instead of replacing them. `attachments` are
  // stored before the operations, which reference them by ID, so a blob and its reference
  // arrive in one request: a document's attachments no reference names are reclaimed when
  // it closes.
  Strict({
    ...base,
    method: T.Literal("batch"),
    ops: operations,
    base: T.Optional(T.String()),
    attachments: T.Optional(T.Array(AttachmentBytesSchema, { minItems: 1 })),
  }),
  Strict({
    ...base,
    method: T.Literal("export"),
    format: T.Enum(ExportFormats),
    output: path,
  }),
]);
/** Every successful result names its method and carries all fields that method promises. */
export const SocketSuccessSchema = T.Union([
  // What an agent reads: the app's descriptor and declared colors (`defaults`), and the
  // document's version, value and effective colors (`theme`).
  Strict({ ok: T.Literal(true), method: T.Literal("get"),
    state: Strict({
      schema: T.Object({}, { additionalProperties: true }),
      defaults: ThemeValuesSchema,
      version: T.String(),
      value: T.Unknown(),
      theme: ThemeValuesSchema,
    }) }),
  // The rows the batch inserted; `get` reads the result.
  Strict({ ok: T.Literal(true), method: T.Literal("batch"), ids: T.Array(T.String()) }),
  Strict({ ok: T.Literal(true), method: T.Literal("export"), output: path }),
  Strict({ ok: T.Literal(true), method: T.Literal("theme.export"), state: Strict({ file: T.String() }) }),
  Strict({ ok: T.Literal(true), method: T.Literal("attachments.list"), state: T.Array(AttachmentInfoSchema) }),
  Strict({ ok: T.Literal(true), method: T.Literal("attachments.read"), state: Strict({ bytes: AttachmentBytesSchema }) }),
]);
/** A classified failure. Unknown outcomes are explicit, never inferred from missing fields. */
export const SocketFailureSchema = Strict({
  ok: T.Literal(false),
  error: T.String(),
  code: OutcomeCodeSchema,
  reason: T.Optional(CoreErrorCodeSchema),
  opIndex: T.Optional(T.Integer({ minimum: 0 })),
});
export const SocketReplySchema = T.Union([...SocketSuccessSchema.anyOf, SocketFailureSchema]);
/** Methods that change the document: a failure leaves an outcome to report. */
export const MutationMethods: ReadonlySet<SocketMethod> = new Set(["batch"]);
/** A live owner's discovery, in the registry (`~/.hitslop/live`): where it listens. */
export const SocketDiscoverySchema = Strict({
  socket: path,
  documentPath: path,
});

export type SocketRequest = T.Static<typeof SocketRequestSchema>;
export type SocketMethod = SocketRequest["method"];
/** A request as a client hands the engine, which adds the protocol it was called with. */
export type HelperRequest = SocketRequest extends infer R ? (R extends unknown ? Omit<R, "protocol"> : never) : never;
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
