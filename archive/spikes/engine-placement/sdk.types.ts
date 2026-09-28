import { defineDocument, s } from "../../packages/document/src/schema";
import type { NativeDocumentClient } from "./sdk";
// New API coverage: invalid author intent must fail at compile time, independently
// of native rejection. Existing SDK type tests do not cover this async surface.
const definition = defineDocument({ title: s.text(), done: s.boolean(), count: s.counter() });
export function authorTypeContract(document: NativeDocumentClient) {
  return document.edit((edit) => {
    edit.set(definition.fields.done, true);
    edit.splice(definition.fields.title, 0, 0, "hello");
    edit.increment(definition.fields.count, 1);
    // @ts-expect-error A boolean field cannot receive a string.
    edit.set(definition.fields.done, "true");
    // @ts-expect-error Text edits require text intent, not scalar assignment.
    edit.set(definition.fields.title, "replacement");
    // @ts-expect-error A boolean is not a counter.
    edit.increment(definition.fields.done);
  });
}
