// Authored schema evaluation must agree in the build and browser. Refuse a mismatched
// or missing descriptor before mounting against the file's document.
import { expect, test } from "bun:test";
import { defineDocument, s } from "@hitslop/document";
import { checkedApp } from "../src/app-module";

test("an app built for another document is refused before it mounts", () => {
  const schema = defineDocument({ title: s.text(), label: s.string({ maxLength: 40 }), note: s.optional(s.string()) });
  // The file stores the descriptor as JSON, with its own key order.
  const stored = JSON.parse(JSON.stringify(schema.descriptor));
  const mount = () => ({});
  expect(checkedApp({ descriptor: schema.descriptor, mount }, stored).mount).toBe(mount);
  expect(() => checkedApp({ mount }, stored)).toThrow("different document");
  const other = defineDocument({ title: s.string() });
  expect(() => checkedApp({ descriptor: other.descriptor, mount }, stored)).toThrow("different document");
  expect(() => checkedApp({}, stored)).toThrow("must export default");
});
