import { Type as T, type Static } from "typebox";
import { Strict } from "./strict";
import { SlopPresentationSchema } from "./manifest";
import { CoreErrorCodeSchema, OwnerPublicationSchema } from "./core";
import {
  AttachmentIDSchema,
  AttachmentBytesSchema,
  AttachmentInfoSchema,
  ThemeValuesSchema,
} from "./values";
import { ErrorTextLimit, PageErrorCodes, PagePayloadLimit, WindowBounds } from "./constants";

const payload = T.String({ minLength: 2, maxLength: PagePayloadLimit });
const sequence = T.Integer({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER });
const text = T.String({ maxLength: ErrorTextLimit });
// The native session supplies lifecycle identity after checking the sending WebView.
// Document payloads stay JSON strings that only the core parses.
export const PageRequests = {
  open: Strict({ method: T.Literal("open") }),
  apply: Strict({ method: T.Literal("apply"), batch: payload }),
  text: Strict({ method: T.Literal("text"), request: payload }),
  flush: Strict({ method: T.Literal("flush") }),
  undo: Strict({ method: T.Literal("undo") }),
  redo: Strict({ method: T.Literal("redo") }),
  config: Strict({ method: T.Literal("config") }),
  "attachments.put": Strict({ method: T.Literal("attachments.put"), bytes: AttachmentBytesSchema }),
  "attachments.read": Strict({
    method: T.Literal("attachments.read"),
    attachmentID: AttachmentIDSchema,
  }),
  "window.resize": Strict({
    method: T.Literal("window.resize"),
    width: T.Integer({ minimum: WindowBounds.minWidth, maximum: WindowBounds.max }),
    height: T.Integer({ minimum: WindowBounds.minHeight, maximum: WindowBounds.max }),
  }),
  ready: Strict({ method: T.Literal("ready") }),
  pageRecovered: Strict({ method: T.Literal("pageRecovered") }),
  failed: Strict({ method: T.Literal("failed"), error: text }),
  pageError: Strict({
    method: T.Literal("pageError"),
    kind: T.Enum(["operation", "application"]),
    error: text,
  }),
} as const;
export const PageRequestSchema = T.Union(Object.values(PageRequests));
export type PageMethod = keyof typeof PageRequests;
export type PageRequest<M extends PageMethod = PageMethod> = {
  [K in PageMethod]: Static<(typeof PageRequests)[K]>;
}[M] & { method: M };

export const PageErrorCodeSchema = T.Enum(PageErrorCodes);
export type PageErrorCode = Static<typeof PageErrorCodeSchema>;
export const PageFailureSchema = Strict({
  ok: T.Literal(false),
  code: PageErrorCodeSchema,
  error: T.String(),
  reason: T.Optional(CoreErrorCodeSchema),
  opIndex: T.Optional(sequence),
});
export type PageFailure = Static<typeof PageFailureSchema>;
export const PageResults = {
  open: Strict({ state: T.String() }),
  apply: Strict({ sequence, ids: T.Array(T.String()) }),
  text: Strict({
    sequence,
    authored: T.String(),
    selectionStart: sequence,
    selectionEnd: sequence,
  }),
  flush: Strict({}),
  undo: Strict({ sequence }),
  redo: Strict({ sequence }),
  config: Strict({
    readOnly: T.Boolean(),
    presentation: SlopPresentationSchema,
    theme: ThemeValuesSchema,
  }),
  "attachments.put": AttachmentInfoSchema,
  "attachments.read": Strict({ bytes: AttachmentBytesSchema }),
  "window.resize": Strict({ width: T.Number(), height: T.Number() }),
  ready: Strict({}),
  pageRecovered: Strict({}),
  failed: Strict({}),
  pageError: Strict({}),
} satisfies Record<PageMethod, T.TObject>;
export type PageResult<M extends PageMethod> = Static<(typeof PageResults)[M]>;
export const PagePushSchema = T.Union([
  Strict({ type: T.Literal("publication"), publication: OwnerPublicationSchema }),
  Strict({ type: T.Literal("resync") }),
]);
export type PagePush = Static<typeof PagePushSchema>;
