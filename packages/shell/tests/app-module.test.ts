// Failure: a custom main.ts passed svelteApp a schema other than the file's, and the app
// mounted against a document it did not describe. Oracle: the shell refuses it before mount.
import { expect, test } from "bun:test";
import { defineDocument, s } from "@hitslop/document";
import { checkedApp } from "../src/app-module";

test("an app built for another document is refused before it mounts", () => {
  const schema = defineDocument({ title: s.text(), label: s.string({ maxLength: 40 }), note: s.optional(s.string()) });
  // The file stores the descriptor as JSON, with its own key order.
  const stored = JSON.parse(JSON.stringify(schema.descriptor));
  const mount = () => ({});
  expect(checkedApp({ descriptor: schema.descriptor, mount }, stored).mount).toBe(mount);
  // An app that declares no descriptor mounts as before.
  expect(checkedApp({ mount }, stored).mount).toBe(mount);
  const other = defineDocument({ title: s.string() });
  expect(() => checkedApp({ descriptor: other.descriptor, mount }, stored)).toThrow("different document");
  expect(() => checkedApp({}, stored)).toThrow("must export default");
});
