import * as T from "typebox";
import {ThemeValuesSchema, AttachmentIDSchema, AttachmentBytesSchema} from "./bridge";
import { SocketLimits } from "./constants";

const identity = T.String({ minLength: 1, maxLength: 128 });
const path = T.String({ minLength: 1, maxLength: 4096 });
const base = {
  id: identity,
  documentPath: path,
};
const mutation = { ...base, epoch: identity };
// Operations are JSON text that only the document core parses: one intent (`apply`) or
// an array of intents (`batch`).
const operations = T.String({ minLength: 2, maxLength: SocketLimits.request });
export const SocketRequestSchema = T.Union([
  T.Object({...base,method:T.Literal("attachments.list")},{additionalProperties:false}),
  T.Object({...base,method:T.Literal("attachments.read"),attachmentID:AttachmentIDSchema},{additionalProperties:false}),
  T.Object({...mutation,method:T.Literal("attachments.put"),bytes:AttachmentBytesSchema},{additionalProperties:false}),
  T.Object({...base,method:T.Literal("theme.get")},{additionalProperties:false}),
  T.Object({...mutation,method:T.Literal("theme.set"),values:ThemeValuesSchema},{additionalProperties:false}),
  T.Object({...mutation,method:T.Literal("theme.reset"),token:T.Optional(T.String({minLength:1,maxLength:128}))},{additionalProperties:false}),
  T.Object({ ...base, method: T.Enum(["hello", "get", "snapshot"]) }, { additionalProperties: false }),
  T.Object({ ...mutation, method: T.Literal("apply"), op: operations }, { additionalProperties: false }),
  T.Object({ ...mutation, method: T.Literal("batch"), ops: operations }, { additionalProperties: false }),
  T.Object({ ...mutation, method: T.Literal("compact") }, { additionalProperties: false }),
  T.Object({ ...mutation, method: T.Literal("export"), format: T.Enum(["png", "pdf"]), output: path }, { additionalProperties: false }),
]);
export const SocketReplySchema = T.Object({
  ok: T.Boolean(),
  epoch: T.Optional(identity),
  /** hello: exact build identity of the owner's document core. */
  coreBuildId: T.Optional(identity),
  state: T.Optional(T.Unknown()),
  /** apply/batch: the IDs of inserted rows (minted or supplied) and the owner sequence. */
  ids: T.Optional(T.Array(T.String())),
  sequence: T.Optional(T.Integer({ minimum: 0 })),
  output: T.Optional(path),
  error: T.Optional(T.String()),
  /** Every code except "failed" means the request was not applied. Absent or "failed": outcome unknown. */
  code: T.Optional(T.Enum(["rejected", "session_changed", "closing", "unavailable", "failed"])),
}, { additionalProperties: false });
/** `state/host.lock`: where a live owner listens. Clients learn the epoch from `hello`. */
export const SocketDiscoverySchema = T.Object({
  socket: path, documentPath: path,
}, { additionalProperties: false });

export type SocketRequest = T.Static<typeof SocketRequestSchema>;
export type SocketReply = T.Static<typeof SocketReplySchema>;
export type SocketDiscovery = T.Static<typeof SocketDiscoverySchema>;
