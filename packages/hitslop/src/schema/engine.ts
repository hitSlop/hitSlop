import * as T from "typebox";
import { Strict } from "./strict";
import { SocketRequestSchema, SocketSuccessSchema, SocketFailureSchema, SocketExportRequestSchema, SocketExportSuccessSchema } from "./socket";
import { CommandCall, CommandSuccess, CommandMetadata } from "./commands";
import { ThemeValuesSchema } from "./values";
import { SlopManifestSchema } from "./manifest";

const path = CommandCall.properties.documentPath;
const object = T.Object({}, { additionalProperties: true });
const size = T.Integer({ minimum: 0 });
const source = T.Enum(["bundled", "installed"]);
const open = Strict({ method: T.Literal("open"), documentPath: path });
const screenshot = Strict({ method: T.Literal("screenshot"), documentPath: path, output: path,
  target: T.Enum(["preview", "icon"]), ifPresent: T.Boolean() });
/** Preserve the schema tuple's static types while removing the transport preflight. */
function withoutProtocol<S extends readonly T.TSchema[]>(schemas: S) {
  return schemas.map(schema => T.Omit(schema, ["protocol"], { additionalProperties: false })) as {
    [K in keyof S]: T.TOmit<S[K], T.TLiteral<"protocol">>
  };
}
const socketRequests = withoutProtocol(SocketRequestSchema.anyOf);
/** One private process boundary. Protocol negotiation stays outside this envelope. */
export const EngineRequestSchema = T.Union([
  ...socketRequests,
  Strict({ method: T.Literal("templates") }),
  Strict({ method: T.Literal("create"), from: path, output: path }),
  Strict({ method: T.Literal("inspect"), file: path }),
  Strict({ method: T.Literal("schema"), file: path }),
  Strict({ method: T.Literal("pack"), stage: path, file: path }),
  // Core reads the format requirements before interpreting any app payload.
  Strict({ method: T.Literal("validateApp"), app: T.Unknown() }),
  Strict({ method: T.Literal("describe"), documentPath: path }),
  Strict({ method: T.Literal("call"), ...CommandCall.properties }),
  open, screenshot,
]);
const openSuccess = Strict({ ok: T.Literal(true), method: T.Literal("open"), documentPath: path });
const screenshotSuccess = Strict({ ok: T.Literal(true), method: T.Literal("screenshot"), output: T.Union([path, T.Null()]) });
export const EngineSuccessSchema = T.Union([
  ...SocketSuccessSchema.anyOf, CommandSuccess,
  Strict({ ok: T.Literal(true), method: T.Literal("templates"), catalog: Strict({
    folders: T.Array(Strict({ source, path })),
    templates: T.Array(Strict({ slug: T.String(), title: T.String(), description: T.String(), categories: T.Array(T.String()), source, path })),
    issues: T.Array(T.String()),
  }) }),
  Strict({ ok: T.Literal(true), method: T.Literal("create"), documentPath: path }),
  Strict({ ok: T.Literal(true), method: T.Literal("inspect"), info: Strict({
    kind: T.Enum(["template", "document"]), packageFormat: T.Integer({ minimum: 1 }), runtimeABI: T.Integer({ minimum: 1 }),
    manifest: SlopManifestSchema, assets: T.Array(Strict({ name: T.String(), bytes: size })),
    artwork: T.Array(Strict({ name: T.String(), bytes: size })), attachments: Strict({ count: size, bytes: size }),
    state: Strict({ checkpointBytes: size, updates: size, updateBytes: size }), bytes: size, storedAssetBytes: size, live: T.Boolean(),
  }) }),
  Strict({ ok: T.Literal(true), method: T.Literal("schema"), schema: object }),
  Strict({ ok: T.Literal(true), method: T.Literal("pack") }),
  Strict({ ok: T.Literal(true), method: T.Literal("validateApp") }),
  Strict({ ok: T.Literal(true), method: T.Literal("describe"), state: Strict({
    manifest: SlopManifestSchema, schema: object, version: T.String(), value: T.Unknown(), theme: ThemeValuesSchema,
    fields: T.Array(Strict({ path: T.Array(T.Union([T.String(), Strict({ id: T.String() }), Strict({ index: size })])),
      kind: T.String(), description: T.Union([T.String(), T.Null()]), operations: T.Array(T.String()) })),
    commands: CommandMetadata,
  }) }),
  openSuccess, screenshotSuccess,
]);
export const EngineReplySchema = T.Union([...EngineSuccessSchema.anyOf, SocketFailureSchema]);
/** The helper only handles operations that need AppKit or WebKit. */
export const NativeRequestSchema = T.Union([open, screenshot, T.Omit(SocketExportRequestSchema, ["protocol"], { additionalProperties: false })]);
export const NativeReplySchema = T.Union([openSuccess, screenshotSuccess, SocketExportSuccessSchema, SocketFailureSchema]);
export type EngineRequest = T.Static<typeof EngineRequestSchema>;
export type EngineMethod = EngineRequest["method"];
export type EngineRequestFor<M extends EngineMethod> = Extract<EngineRequest, { method: M }>;
export type EngineSuccess = T.Static<typeof EngineSuccessSchema>;
export type EngineReply = T.Static<typeof EngineReplySchema>;
export type EngineReplyFor<M extends EngineMethod> = Extract<EngineSuccess, { method: M }> | T.Static<typeof SocketFailureSchema>;
