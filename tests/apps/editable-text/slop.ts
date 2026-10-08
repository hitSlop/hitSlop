import { defineSlop } from "hitslop";
import App from "./App.svelte";
import document from "./schema";
export default defineSlop({
  slug: "editable-text-fixture", title: "EditableText fixture", description: "SDK browser contracts.",
  author: { name: "Test" }, categories: ["utilities"],
  window: { kind: "standard", width: 400, height: 500 },
  theme: {}, document, view: App,
  initial: { first: "First field", second: "A second field with words" },
});
