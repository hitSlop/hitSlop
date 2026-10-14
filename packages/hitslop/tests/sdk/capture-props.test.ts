// Gap: static views are functions of their props. Preview, Export, Icon and every widget
// viewport receive the saved snapshot, frozen all the way down, and attachment URLs with the
// host's refusal semantics. Oracle: the props a server-compiled view records, and its frames.
import { afterAll, expect, test } from "bun:test";
import { compile } from "svelte/compiler";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { viewRenderer } from "../../src/sdk/app/capture-render";
import { widgetRenderer } from "../../src/sdk/app/widget-render";

const folder = await mkdtemp(join(import.meta.dir, ".build-test-capture-props-"));
afterAll(() => rm(folder, { recursive: true, force: true }));
/** A server-compiled component module. */
async function component(name: string, source: string) {
  const path = join(folder, `${name}.js`);
  await writeFile(path, compile(source, { filename: `${name}.svelte`, generate: "server" }).js.code);
  return await import(path);
}
// Records its props and shows the saved title and stored photo.
const View = await component("View", `<script module>export const seen = [];</script>
<script>let props = $props(); seen.push(props);</script>
<h1>{props.snapshot.title}</h1>{#if props.snapshot.cover}<img src={props.attachmentURL(props.snapshot.cover.id)} alt="" />{/if}`);
const Mutating = await component("Mutating", `<script>let { snapshot } = $props(); snapshot.rows[0].text = "Changed";</script><p>Changed</p>`);

const snapshot = { title: "Saved", cover: { id: "photo" }, rows: [{ $id: "r1", text: "Row", tags: ["a"] }] };
const resources = { photo: "data:image/png;base64,iVBORw0KGgo=" };
const view = (kind: string, values: object = {}) =>
  JSON.stringify({ kind, format: "png", width: 320, height: 240, snapshot, theme: {}, resources, ...values });
const viewports = [
  { family: "small", width: 164, height: 164 },
  { family: "medium", width: 344, height: 164 },
  { family: "large", width: 344, height: 354 },
];
function expectDeeplyFrozen(value: unknown) {
  if (!value || typeof value !== "object") return;
  expect(Object.isFrozen(value)).toBe(true);
  for (const child of Object.values(value)) expectDeeplyFrozen(child);
}

test("Preview, Export and Icon receive the frozen snapshot, attachment URLs and their mode", () => {
  for (const kind of ["preview", "export", "icon"]) {
    const [frame] = JSON.parse(viewRenderer(View.default, "")(view(kind)));
    const props = View.seen.at(-1);
    expect(Object.keys(props).sort()).toEqual(["attachmentURL", "mode", "snapshot"]);
    expect(props.mode).toBe(kind);
    expect(props.snapshot).toEqual(snapshot);
    expectDeeplyFrozen(props.snapshot);
    expect(frame.html).toStartWith(
      `<html data-slop-capture="${kind === "icon" ? "icon" : "static"}"><body><div data-slop-capture-target="${kind}">`,
    );
    expect(frame.html).toContain("<h1>Saved</h1>");
    expect(frame.html).toContain('<img src="slop-attachment:photo"');
  }
});

test("each widget viewport receives one frozen snapshot, attachment URLs and its family and size", () => {
  const frames = JSON.parse(
    widgetRenderer(View.default, "")(JSON.stringify({ snapshot, theme: {}, resources, viewports })),
  );
  expect(frames.map(({ width, height }: { width: number; height: number }) => [width, height])).toEqual(
    viewports.map(({ width, height }) => [width, height]),
  );
  const seen = View.seen.slice(-3);
  expect(seen.map(({ family, width, height }: (typeof viewports)[number]) => ({ family, width, height }))).toEqual(viewports);
  expect(Object.keys(seen[0]).sort()).toEqual(["attachmentURL", "family", "height", "snapshot", "width"]);
  // Frozen in place: every viewport reads the same objects.
  expect(new Set(seen.map(({ snapshot }: { snapshot: object }) => snapshot)).size).toBe(1);
  expectDeeplyFrozen(seen[0].snapshot);
  expect(seen[0].attachmentURL("photo")).toBe("slop-attachment:photo");
  for (const frame of frames) expect(frame.html).toContain('<img src="slop-attachment:photo"');
});

test("a view cannot change its snapshot", () => {
  expect(() => viewRenderer(Mutating.default, "")(view("export"))).toThrow(TypeError);
  expect(() =>
    widgetRenderer(Mutating.default, "")(JSON.stringify({ snapshot, theme: {}, resources, viewports })),
  ).toThrow(TypeError);
  viewRenderer(View.default, "")(view("preview"));
  const { snapshot: frozen } = View.seen.at(-1);
  expect(() => (frozen.title = "Changed")).toThrow(TypeError);
  expect(() => frozen.rows.push({ $id: "r2", text: "Added", tags: [] })).toThrow(TypeError);
  expect(() => (frozen.cover.id = "other")).toThrow(TypeError);
  expect(() => frozen.rows[0].tags.push("b")).toThrow(TypeError);
});

test("attachment URLs resolve only the supported images the request supplies", () => {
  viewRenderer(View.default, "")(view("export"));
  const { attachmentURL } = View.seen.at(-1);
  expect(attachmentURL("photo")).toBe("slop-attachment:photo");
  // The host supplies stored images only: a recording or a removed file has no entry.
  for (const id of ["recording", "toString", "", 7, undefined])
    expect(() => attachmentURL(id)).toThrow(`Capture attachment ${id} is missing or is not a supported image`);
  // An entry that is not an image's data is refused too, and so is a missing inventory.
  for (const supplied of [{ photo: "" }, { photo: null }, { photo: 7 }, null]) {
    expect(() => viewRenderer(View.default, "")(view("export", { resources: supplied }))).toThrow(
      "Capture attachment photo is missing or is not a supported image",
    );
    const { attachmentURL } = View.seen.at(-1);
    expect(() => attachmentURL("photo")).toThrow("is missing or is not a supported image");
  }
});
