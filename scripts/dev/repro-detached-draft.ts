// Manual audit reproduction, not a new CI contract. Runs the production page shell
// against a transport whose edit outcome cannot be confirmed.
// bun scripts/dev/repro-detached-draft.ts
import { OwnerDocument, type OwnerTransport } from "../../packages/tinys/src/shell/owner/document";
import { defineDocument, s } from "../../packages/tinys/src/sdk/schema";
import { DocumentError, isDocumentError } from "../../packages/tinys/src/sdk/errors";

const before = "before";
const draft = "important unsaved draft";
const reported: { code?: string; message: string }[] = [];
let sends = 0;
let flushes = 0;
const transport: OwnerTransport = {
  open: async () => ({ sequence: 0, version: "v0", value: { value: before }, theme: {} }),
  apply: async () => {
    sends++;
    throw new DocumentError("unknown_outcome", "connection lost");
  },
  flush: async () => { flushes++; },
  undo: async () => ({ sequence: 0 }),
  redo: async () => ({ sequence: 0 }),
  onPush() {},
};
const definition = defineDocument({ value: s.text() });
const doc = await OwnerDocument.open(definition, transport, (error) => reported.push({
  code: isDocumentError(error) ? error.code : undefined,
  message: error instanceof Error ? error.message : String(error),
}));
class Field extends EventTarget {
  value = "";
  selectionStart = 0;
  selectionEnd = 0;
  disabled = false;
  setSelectionRange(start: number, end: number) { this.selectionStart = start; this.selectionEnd = end; }
  blur() {}
}
const input = new Field();
const binding = doc.bindText(input as unknown as HTMLInputElement, doc.fields.value);
input.value = draft;
input.selectionStart = input.selectionEnd = draft.length;
input.dispatchEvent(new Event("input"));
binding.destroy();
await new Promise((resolve) => setTimeout(resolve, 20));
await doc.prepareClose();
console.log(JSON.stringify({
  closeSucceeded: true, sends, flushes, savedValue: doc.current.value,
  reported, draftIncludedInRecoveryReport: reported.some((error) => error.message.includes(draft)),
}, null, 2));
