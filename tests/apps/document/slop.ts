import { defineSlop } from "hitslop";
import App from "./App.svelte";
import Export from "./Export.svelte";
import Icon from "./Icon.svelte";
import document from "./schema";
import * as commands from "./commands";
import "./styles.css";

export default defineSlop({
  slug: "document-fixture", title: "Document fixture", description: "Native and authoring contracts.",
  author: { name: "Test" }, categories: ["utilities"],
  window: { kind: "standard", width: 480, height: 620 },
  theme: { surface: "#e98996", ink: "#202020", accent: "#335577" },
  document, view: App, export: Export, icon: Icon, commands,
  initial: { title: "Fixture title", tasks: [
    { text: "First row", done: true }, { text: "Second row" }, { text: "Third row" },
  ] },
});
