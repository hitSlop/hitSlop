// Fast-tier replay of the shared host fixtures (tests/fixtures) through the WASM core,
// so Linux CI covers what the macOS host path test checks natively.
import { expect, test } from "bun:test";
import { readdir } from "node:fs/promises";
import { OwnerDocument } from "../src/owner/document";
import { fromDescriptor } from "../src/schema";

const root = new URL("../../../tests/fixtures/", import.meta.url);
const wasm = await import(new URL("../../../generated/v1/core/wasm/hitslop_core_wasm.js", import.meta.url).href);
wasm.initSync({
  module: await Bun.file(new URL("../../../generated/v1/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url)).bytes(),
});

for (const name of (await readdir(root)).sort()) {
  test(`fixture ${name} opens and replays its scenario`, async () => {
    const at = (path: string) => Bun.file(new URL(`${name}/${path}`, root)).json();
    const descriptor = await at("document/state.schema.json");
    const core = wasm.WasmDocument.create(JSON.stringify(descriptor), JSON.stringify(await at("document/initial.json")));
    try {
      const doc = await OwnerDocument.open(fromDescriptor(descriptor), {
        state: async () => JSON.parse(core.snapshot()),
        apply: async ({ batch }) => JSON.parse(core.apply(JSON.stringify(batch))),
        text: async (request) => JSON.parse(core.text(JSON.stringify(request))),
        releaseDraft: async (id) => core.release_draft(id),
        flush: async () => {},
      });
      expect(doc.current).toEqual(await at("expected.json"));
      const scenario = await at("scenario.json");
      for (const { path, method, args } of scenario.handles) {
        let handle: any = doc.fields;
        for (const key of path) handle = handle[key];
        await handle[method](...args);
      }
      expect(doc.current).toEqual(scenario.expected);
    } finally {
      core.free();
    }
  });
}
