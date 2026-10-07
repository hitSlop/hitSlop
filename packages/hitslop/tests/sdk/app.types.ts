import { defineDocument, defineSlop, s } from "../../src/sdk/schema";
import type { Component } from "svelte";

// The explicit declaration uses Rust's metadata/window types while document inputs
// retain SDK inference. These functions are checked by tsc, never executed.
function declarations(view: Component) {
  const document = defineDocument({ title: s.string() });
  const fields = {
    slug: "fixture", title: "Fixture", description: "A typed declaration",
    author: { name: "Test" }, categories: ["utilities"] as const,
    theme: {}, document, initial: { title: "Initial" }, view,
  };
  defineSlop({ ...fields, window: { kind: "standard", width: 320, height: 320, shape: { path: "M0 0L1 0L1 1Z", viewBox: [1, 1], fillRule: "evenodd" } } });
  defineSlop({ ...fields, window: { kind: "skin", width: 320, height: 320, image: "/assets/skin.png" } });
  // @ts-expect-error Skin windows require an imported image URL.
  defineSlop({ ...fields, window: { kind: "skin", width: 320, height: 320 } });
  const mixed = { kind: "skin" as const, width: 320, height: 320, image: "/assets/skin.png", resizable: true };
  // @ts-expect-error A skin cannot carry standard-window flags, even through a variable.
  defineSlop({ ...fields, window: mixed });
  // @ts-expect-error Initial values follow the document definition.
  defineSlop({ ...fields, initial: { title: 7 }, window: { kind: "standard", width: 320, height: 320 } });
  // @ts-expect-error Catalog categories come from the Rust enum.
  defineSlop({ ...fields, categories: ["unknown"], window: { kind: "standard", width: 320, height: 320 } });
}
void declarations;
