import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "document-fixture",
  title: "Document fixture",
  description: "Native and authoring contracts.",
  author: { name: "Test" },
  categories: ["utilities"],
  window: { kind: "standard", width: 480, height: 620 },
  theme: { surface: "#e98996", ink: "#202020", accent: "#335577" },
});
