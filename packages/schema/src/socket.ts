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
  Strict({ ...mutation, method: T.Literal("compact") }),
  Strict({
    ...mutation,
    method: T.Literal("export"),
    format: T.Enum(ExportFormats),
    output: path,
  }),
]);
export const SocketReplySchema = Strict({
  ok: T.Boolean(),
  epoch: T.Optional(identity),
  /** hello: exact build identity of the owner's document core. */
  coreBuildId: T.Optional(identity),
  /** The method's result (`SocketResults`), as the core's JSON. */
  state: T.Optional(T.Unknown()),
  /** batch: the IDs of inserted rows (minted or supplied) and the owner sequence. */
  ids: T.Optional(T.Array(T.String())),
  sequence: T.Optional(T.Integer({ minimum: 0 })),
  output: T.Optional(path),
  error: T.Optional(T.String()),
  /** A failure's outcome (`OutcomeCodes`); absent means unknown. */
  code: T.Optional(OutcomeCodeSchema),
  /** A refusal's core error code, and the intent it refused. */
  reason: T.Optional(CoreErrorCodeSchema),
  opIndex: T.Optional(T.Integer({ minimum: 0 })),
});
/** What each method's successful reply must carry beside `ok`; a reply without it is an
 * unknown outcome, never a result with defaults. */
const theme = T.Object({ state: ThemeStateSchema });
export const SocketResults = {
  get: T.Object({ state: Strict({ schema: T.Object({}, { additionalProperties: true }), state: OwnerStateSchema }) }),
  batch: T.Object({ ids: T.Array(T.String()), sequence: T.Integer({ minimum: 0 }) }),
  compact: T.Object({}),
  export: T.Object({ output: path }),
  "theme.get": theme,
  "theme.set": theme,
  "theme.reset": theme,
  "theme.import": theme,
  /** The theme file's text, as the core writes it. */
  "theme.export": T.Object({ state: Strict({ file: T.String() }) }),
  "attachments.list": T.Object({ state: T.Array(AttachmentInfoSchema) }),
  "attachments.read": T.Object({ state: Strict({ bytes: AttachmentBytesSchema }) }),
  "attachments.put": T.Object({ state: AttachmentInfoSchema }),
} as const;
/** A live owner's discovery, in the registry (`~/.hitslop/live`): where it listens. Clients
 * learn the epoch from `hello`. */
export const SocketDiscoverySchema = Strict({
  socket: path,
  documentPath: path,
});

export type SocketRequest = T.Static<typeof SocketRequestSchema>;
export type SocketReply = T.Static<typeof SocketReplySchema>;
export type SocketDiscovery = T.Static<typeof SocketDiscoverySchema>;
