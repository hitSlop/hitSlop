import { defineDocument, defineSlop, s } from "../../src/sdk/schema";
import type { Component } from "svelte";
import { attachments } from "../../src/sdk/app/attachments";
import { svelteApp } from "../../src/sdk/app/store.svelte";

function declarations(view: Component) {
  const fields = {
    slug: "fixture", title: "Fixture", description: "A typed declaration",
    author: { name: "Test" }, categories: ["utilities"] as const, theme: {},
  };
  const window = { kind: "standard", width: 320, height: 320 } as const;
  defineSlop({ ...fields, window: { ...window, shape: { path: "M0 0L1 0L1 1Z", viewBox: [1, 1], fillRule: "evenodd" } } });
  defineSlop({ ...fields, window: { kind: "skin", width: 320, height: 320, image: "/assets/skin.png" } });
  // @ts-expect-error Skin windows require an imported image URL.
  defineSlop({ ...fields, window: { kind: "skin", width: 320, height: 320 } });
  const mixed = { kind: "skin" as const, width: 320, height: 320, image: "/assets/skin.png", resizable: true };
  // @ts-expect-error A skin cannot carry standard-window flags, even through a variable.
  defineSlop({ ...fields, window: mixed });
  // @ts-expect-error Initial values belong in schema.ts.
  defineSlop({ ...fields, window, initial: {} });
  // @ts-expect-error Components are discovered by filename.
  defineSlop({ ...fields, window, view });
  // @ts-expect-error Commands are discovered by filename.
  defineSlop({ ...fields, window, commands: {} });
  // @ts-expect-error Catalog categories come from the Rust enum.
  defineSlop({ ...fields, categories: ["unknown"], window });
  defineDocument({ title: s.string() }, { initial: { title: "Initial" } });
  defineDocument({ title: s.text(), count: s.counter(), note: s.optional(s.string()), done: s.boolean({default:false}) });
  // @ts-expect-error Required fields need initial values.
  defineDocument({ title: s.string() });
  // @ts-expect-error Initial does not influence descriptor inference.
  defineDocument({ title: s.string() }, { initial: { title: 7 } });
  // @ts-expect-error An undefined default does not make a field omittable.
  defineDocument({ done: s.boolean({default:undefined}) });
}
void declarations;

async function collectors(file: File) {
  const doc = defineDocument({ title: s.string(), photo: s.optional(s.string()) }, { initial: {title:""} });
  const id: { id: string } = await doc.change(() => ({ id: "kept" }));
  // @ts-expect-error A change() collector cannot be async.
  await doc.change(async tx => { await tx.fields.title.set("Later"); });
  await attachments.import<typeof doc.descriptor>(file, (tx, ref) => tx.fields.photo.set(ref.id));
  // @ts-expect-error An attachment's reference collector cannot be async.
  await attachments.import<typeof doc.descriptor>(file, async (tx, ref) => tx.fields.photo.set(ref.id));
  return id;
}
void collectors;

function exportRoles(plain: Component<Record<string, never>>, capture: Component<{ mode: "preview" | "export" }>, needy: Component<{ task: string }>) {
  const schema = defineDocument({ title: s.text() });
  svelteApp(plain, {schema, export:plain});
  svelteApp(plain, {schema, export:capture});
  // @ts-expect-error An export receives only its capture mode.
  svelteApp(plain, {schema, export:needy});
}
void exportRoles;
