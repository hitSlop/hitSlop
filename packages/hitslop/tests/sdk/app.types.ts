import { defineDocument, defineSlop, s } from "../../src/sdk/schema";
import type { Component } from "svelte";
import { attachments } from "../../src/sdk/app/attachments";

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

// Collectors run synchronously; inside them writes collect into one batch.
async function collectors(file: File) {
  const doc = defineDocument({ title: s.string(), photo: s.optional(s.string()) });
  const id: { id: string } = await doc.change(() => ({ id: "kept" }));
  // @ts-expect-error A change() collector cannot be async.
  await doc.change(async tx => { await tx.fields.title.set("Later"); });
  await attachments.import<typeof doc.descriptor>(file, (tx, ref) => tx.fields.photo.set(ref.id));
  // @ts-expect-error An attachment's reference collector cannot be async.
  await attachments.import<typeof doc.descriptor>(file, async (tx, ref) => tx.fields.photo.set(ref.id));
  return id;
}
void collectors;

// A capture component may declare its host-provided mode or take no props; one that needs
// other props cannot be an export.
function exportRoles(
  plain: Component<Record<string, never>>,
  capture: Component<{ mode: "preview" | "export" }>,
  needy: Component<{ task: string }>,
) {
  const document = defineDocument({ title: s.string() });
  const fields = {
    slug: "fixture", title: "Fixture", description: "A typed declaration",
    author: { name: "Test" }, categories: ["utilities"] as const,
    theme: {}, document, initial: { title: "Initial" }, view: plain,
    window: { kind: "standard", width: 320, height: 320 } as const,
  };
  defineSlop({ ...fields, export: plain });
  defineSlop({ ...fields, export: capture });
  // @ts-expect-error An export receives only its capture mode.
  defineSlop({ ...fields, export: needy });
}
void exportRoles;
