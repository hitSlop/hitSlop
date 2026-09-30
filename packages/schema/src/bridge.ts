import * as T from "typebox";
import { SlopPresentationSchema } from "./manifest";
import { AttachmentIdPattern, AttachmentLimits, ErrorTextLimit, ThemeTokenRule, WindowBounds, base64Length } from "./constants";
export { AttachmentLimits, base64Length };
const text = T.String({ maxLength: ErrorTextLimit });
const strict = { additionalProperties: false };
export const AttachmentIDSchema = T.String({
  pattern: AttachmentIdPattern,
  minLength: 64,
  maxLength: 64,
});
export const AttachmentBytesSchema = T.String({ maxLength: base64Length(AttachmentLimits.file) });
export const ThemeValuesSchema = T.Record(
  T.String({ pattern: ThemeTokenRule.name }),
  T.String({ minLength: 1, maxLength: ThemeTokenRule.valueLength }),
  strict,
);
export const BridgeMethods = {
  "attachments.put": T.Object({ method: T.Literal("attachments.put"), bytes: AttachmentBytesSchema }, strict),
  "attachments.read": T.Object({ method: T.Literal("attachments.read"), attachmentID: AttachmentIDSchema }, strict),
  "attachments.list": T.Object({ method: T.Literal("attachments.list") }, strict),
  "theme.load": T.Object({ method: T.Literal("theme.load") }, strict),
  pageRecovered: T.Object({ method: T.Literal("pageRecovered") }, strict),
  "window.resize": T.Object({
    method: T.Literal("window.resize"),
    width: T.Integer({ minimum: WindowBounds.minWidth, maximum: WindowBounds.max }),
    height: T.Integer({ minimum: WindowBounds.minHeight, maximum: WindowBounds.max }),
  }, strict),
  config: T.Object({ method: T.Literal("config") }, strict),
  ready: T.Object({ method: T.Literal("ready") }, strict),
  failed: T.Object({ method: T.Literal("failed"), error: text }, strict),
  pageError: T.Object({
    method: T.Literal("pageError"),
    kind: T.Enum(["operation", "application"]),
    error: text,
  }, strict),
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
    presentation: SlopPresentationSchema,
  }),
  "attachments.put": attachmentInfo,
  "attachments.read": T.Object({ bytes: T.String() }),
  "attachments.list": T.Object({ files: T.Array(attachmentInfo) }),
  "theme.load": T.Object({ values: ThemeValuesSchema }),
  "window.resize": T.Object({ width: T.Number(), height: T.Number() }),
  pageRecovered: T.Object({}),
  ready: T.Object({}),
  failed: T.Object({}),
  pageError: T.Object({}),
} satisfies Record<BridgeMethod, T.TObject>;
export type BridgeReply<M extends BridgeMethod> = T.Static<(typeof BridgeReplies)[M]>;
