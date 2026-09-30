import * as T from "typebox";
export const protocolVersion = 1;
const text = T.String({ maxLength: 4096 });
export const AttachmentIDSchema = T.String({
  pattern: "^[a-f0-9]{64}$",
  minLength: 64,
  maxLength: 64,
});
export const AttachmentBytesSchema = T.String({ maxLength: 13981016 });
export const ThemeValuesSchema = T.Record(
  T.String({ pattern: "^[a-zA-Z][a-zA-Z0-9-]*$" }),
  T.String({ minLength: 1, maxLength: 4096 }),
);
export const BridgeMethods = {
  "attachments.put": T.Object({
    method: T.Literal("attachments.put"),
    bytes: AttachmentBytesSchema,
  }),
  "attachments.read": T.Object({
    method: T.Literal("attachments.read"),
    attachmentID: AttachmentIDSchema,
  }),
  "attachments.list": T.Object({ method: T.Literal("attachments.list") }),
  "theme.load": T.Object({ method: T.Literal("theme.load") }),
  "theme.save": T.Object({ method: T.Literal("theme.save"), values: ThemeValuesSchema }),
  runtimeRecovered: T.Object({ method: T.Literal("runtimeRecovered") }),
  "window.resize": T.Object({
    method: T.Literal("window.resize"),
    width: T.Integer({ minimum: 240, maximum: 4096 }),
    height: T.Integer({ minimum: 180, maximum: 4096 }),
  }),
  config: T.Object({ method: T.Literal("config") }),
  ready: T.Object({ method: T.Literal("ready") }),
  failed: T.Object({ method: T.Literal("failed"), error: text }),
  runtimeError: T.Object({
    method: T.Literal("runtimeError"),
    kind: T.Enum(["operation", "application"]),
    error: text,
  }),
} as const;
export const BridgeRequestSchema = T.Union(Object.values(BridgeMethods));

export type BridgeMethod = keyof typeof BridgeMethods;
export type BridgeRequest<M extends BridgeMethod = BridgeMethod> = {
  [K in BridgeMethod]: T.Static<(typeof BridgeMethods)[K]>;
}[M] & { method: M };

const attachmentInfo = T.Object({ id: AttachmentIDSchema, byteLength: T.Integer() });
/** Successful host replies. Refusals are handled by hostCall before returning. */
export const BridgeReplies = {
  config: T.Object({
    epoch: T.String(),
    view: T.String(),
    documentID: T.String(),
    readOnly: T.Boolean(),
    presentation: T.Object({
      width: T.Number(),
      height: T.Number(),
      resizable: T.Boolean(),
      shape: T.Enum(["rounded", "ellipse", "capsule"]),
      mode: T.Enum(["standard", "transparent", "skin"]),
    }),
  }),
  "attachments.put": attachmentInfo,
  "attachments.read": T.Object({ bytes: T.String() }),
  "attachments.list": T.Object({ files: T.Array(attachmentInfo) }),
  "theme.load": T.Object({ values: ThemeValuesSchema }),
  "window.resize": T.Object({ width: T.Number(), height: T.Number() }),
  "theme.save": T.Object({}),
  runtimeRecovered: T.Object({}),
  ready: T.Object({}),
  failed: T.Object({}),
  runtimeError: T.Object({}),
} satisfies Record<BridgeMethod, T.TObject>;
export type BridgeReply<M extends BridgeMethod> = T.Static<(typeof BridgeReplies)[M]>;
