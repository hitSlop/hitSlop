import type { At, Handle } from "./handle-types";
import type { ObjectNode, Path } from "./schema";
export type SaveStatus = "saved" | "saving" | "save-failed";
export type DocumentEvent =
  | { readonly kind: "change"; readonly origin: "local" | "host" | "remote" }
  | { readonly kind: "status" };
export type Scope<N extends ObjectNode> = { readonly fields: Handle<N>; readonly at: At };
export type Issue = {
  readonly path: Path;
  readonly kind: "invalid" | "constraint" | "unknown-field" | "identity";
  readonly detail: string;
};
export type AttachmentInfo = { id: string; byteLength: number };
export type AttachmentRef = AttachmentInfo & { name: string; mimeType: string };
export type CaptureMode = "preview" | "export" | "icon";
