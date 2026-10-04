import { Strict } from "./strict";
import * as T from "typebox";
import { ThemeTokenSchema, ThemeValuesSchema, AttachmentIDSchema, AttachmentBytesSchema } from "./values";
import { SocketLimits, ThemeFileLimit } from "./constants";

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
  Strict({ ...mutation, method: T.Literal("compact") }),
  Strict({
    ...mutation,
    method: T.Literal("export"),
    format: T.Enum(["png", "pdf"]),
    output: path,
  }),
]);
export const SocketReplySchema = Strict({
  ok: T.Boolean(),
  epoch: T.Optional(identity),
  /** hello: exact build identity of the owner's document core. */
  coreBuildId: T.Optional(identity),
  state: T.Optional(T.Unknown()),
  /** batch: the IDs of inserted rows (minted or supplied) and the owner sequence. */
  ids: T.Optional(T.Array(T.String())),
  sequence: T.Optional(T.Integer({ minimum: 0 })),
  output: T.Optional(path),
  error: T.Optional(T.String()),
  /** Every code except "failed" means the request was not applied. Absent or "failed": outcome unknown. */
  code: T.Optional(T.Enum(["rejected", "session_changed", "closing", "unavailable", "failed"])),
});
/** A live owner's discovery, in the registry (`~/.hitslop/live`): where it listens. Clients
 * learn the epoch from `hello`. */
export const SocketDiscoverySchema = Strict({
  socket: path,
  documentPath: path,
});

export type SocketRequest = T.Static<typeof SocketRequestSchema>;
export type SocketReply = T.Static<typeof SocketReplySchema>;
export type SocketDiscovery = T.Static<typeof SocketDiscoverySchema>;
