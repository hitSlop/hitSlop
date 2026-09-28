// Gap: Rust tests cannot prove renderer promises, collector lifetime, or immutable
// snapshots. Oracle: literal authored outcomes through the real WASM binding.
import { expect, test } from "bun:test";
import { OwnerDocument, type OwnerTransport } from "../src/owner/document";
import { defineDocument, s } from "../src/schema";
import { projection } from "../src/owner/projection";
import { Check } from "typebox/value";
import { OwnerStateSchema, OwnerPublicationSchema } from "@hitslop/schema/owner";
const moduleURL = new URL("../../../generated/v1/core/wasm/hitslop_core_wasm.js", import.meta.url);
const wasm = await import(moduleURL.href);
wasm.initSync({
  module: await Bun.file(
    new URL("../../../generated/v1/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url),
  ).bytes(),
});
const definition = defineDocument({
  title: s.text(),
  done: s.boolean(),
  hits: s.counter(),
  rows: s.list(s.object({ text: s.text(), done: s.boolean() })),
});
const initial = {
  title: "Hello",
  done: false,
  hits: 0,
  rows: [
    { $id: "a", text: "First", done: false },
    { $id: "b", text: "Second", done: false },
  ],
};

test("WASM binding executes literal core fixtures and replays native-compatible bytes", async () => {
  const fixture = await Bun.file(
    new URL("../../../crates/hitslop-core/fixtures/checklist.json", import.meta.url),
  ).json();
  for (const scenario of fixture.scenarios) {
    const core = wasm.WasmDocument.create(
      JSON.stringify(fixture.schema),
      JSON.stringify(scenario.initial ?? fixture.initial),
    );
    try {
      const before = core.snapshot();
      expect(Check(OwnerStateSchema, JSON.parse(before))).toBe(true);
      const seed = core.checkpoint();
      const version = core.version();
      const intents = scenario.intents.map((op: any) =>
        op.base === "$current" ? { ...op, base: version } : op,
      );
      if (scenario.error) {
        expect(() => core.apply(JSON.stringify({ intents }))).toThrow(scenario.error);
        expect(core.snapshot()).toBe(before);
      } else {
        const reply = JSON.parse(core.apply(JSON.stringify({ intents })));
        expect(Check(OwnerPublicationSchema, reply)).toBe(true);
        expect(JSON.parse(core.snapshot()).value).toEqual(scenario.after);
        const reopened = wasm.WasmDocument.open(JSON.stringify(fixture.schema), seed);
        try {
          reopened.import_updates(core.export_since(version));
          expect(JSON.parse(reopened.snapshot()).value).toEqual(scenario.after);
        } finally {
          reopened.free();
        }
      }
    } finally {
      core.free();
    }
  }
});
async function open() {
  const core = wasm.WasmDocument.create(
    JSON.stringify(definition.descriptor),
    JSON.stringify(initial),
  );
  const errors: unknown[] = [];
  const transport: OwnerTransport = {
    state: async () => JSON.parse(core.snapshot()),
    apply: async ({ batch }) => JSON.parse(core.apply(JSON.stringify(batch))),
    text: async (request) => JSON.parse(core.text(JSON.stringify(request))),
    releaseDraft: async (id) => core.release_draft(id),
    flush: async () => {},
  };
  const doc = await OwnerDocument.open(definition, transport, (error) => errors.push(error));
  return { core, transport, doc, errors };
}

test("ordinary writes resolve after publication and preserve unaffected snapshot identity", async () => {
  const { core, transport, doc } = await open();
  try {
    let release!: () => void, entered!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const started = new Promise<void>((resolve) => (entered = resolve));
    const apply = transport.apply;
    transport.apply = async (request) => {
      entered();
      await gate;
      return apply(request);
    };
    const before = doc.current;
    const pending = doc.fields.rows.item("a").done.set(true);
    await started;
    expect(doc.current).toBe(before);
    expect(doc.status).toBe("pending");
    release();
    await pending;
    expect(doc.current.rows[0]!.done).toBe(true);
    expect(doc.current.rows[1]).toBe(before.rows[1]);
    expect(before.rows[0]!.done).toBe(false);
    expect(Object.isFrozen(doc.current.rows[0])).toBe(true);
    expect(doc.status).toBe("pending");
    await doc.flush();
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});

class Field extends EventTarget {
  value = "";
  selectionStart = 0;
  selectionEnd = 0;
  setSelectionRange(start: number, end: number) {
    this.selectionStart = start;
    this.selectionEnd = end;
  }
  blur() {}
}
test("text composition stays local until commit and flush drains the real core reply", async () => {
  const { core, doc } = await open();
  const field = new Field();
  const binding = doc.bindText(field as unknown as HTMLInputElement, doc.fields.title);
  try {
    field.dispatchEvent(new Event("compositionstart"));
    field.value = "Hello 日本😀";
    field.setSelectionRange(field.value.length, field.value.length);
    field.dispatchEvent(new Event("input"));
    expect(doc.current.title).toBe("Hello");
    await expect(doc.flush()).rejects.toThrow("composition_pending");
    field.dispatchEvent(new Event("compositionend"));
    await doc.flush();
    expect(doc.current.title).toBe("Hello 日本😀");
    expect(field.value).toBe("Hello 日本😀");
    expect(doc.status).toBe("saved");
  } finally {
    binding.destroy();
    core.free();
  }
});

test("collectors insert then address minted IDs synchronously and resolve only after the batch", async () => {
  const { core, doc } = await open();
  try {
    const inserted = await doc.change((tx) => {
      const result = tx.fields.rows.insert({ text: "New", done: false });
      tx.fields.rows.item(result.id).done.set(true);
      tx.fields.rows.item(result.id).text.replace("New title");
      tx.fields.rows.item(result.id).text.replace("Final");
      tx.fields.hits.increment(4);
      tx.fields.hits.decrement();
      return result;
    });
    expect(JSON.parse(JSON.stringify(doc.current.rows.at(-1)))).toEqual({
      $id: inserted.id,
      text: "Final",
      done: true,
    });
    expect(doc.current.hits).toBe(3);
    await doc.at(doc.current.rows[0]!).done.set(true);
    expect(doc.current.rows[0]!.done).toBe(true);
  } finally {
    core.free();
  }
});

test("previews stay outside the core until flush and survive a rejected commit", async () => {
  const { core, doc, transport } = await open();
  try {
    doc.fields.done.preview(true);
    expect(doc.current.done).toBe(true);
    expect(JSON.parse(core.snapshot()).value.done).toBe(false);
    expect(doc.status).toBe("pending");
    const apply = transport.apply;
    transport.apply = async () => {
      throw Error("rejected");
    };
    await expect(doc.flush()).rejects.toThrow("rejected");
    expect(doc.current.done).toBe(true);
    expect(JSON.parse(core.snapshot()).value.done).toBe(false);
    transport.apply = apply;
    await doc.flush();
    expect(JSON.parse(core.snapshot()).value.done).toBe(true);
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});

test("throwing, async, nested and escaped collectors never submit their staged changes", async () => {
  const { core, doc, transport } = await open();
  try {
    let sent = 0;
    const apply = transport.apply;
    transport.apply = (request) => {
      sent++;
      return apply(request);
    };
    await expect(
      doc.change((tx) => {
        tx.fields.done.set(true);
        throw Error("stop");
      }),
    ).rejects.toThrow("stop");
    await expect(
      doc.change(async (tx) => {
        tx.fields.done.set(true);
      }),
    ).rejects.toThrow("synchronous");
    await expect(doc.change(() => doc.change(() => {}))).rejects.toThrow("Nested");
    await expect(doc.change(() => doc.fields.done.set(true))).rejects.toThrow("tx handles");
    expect(sent).toBe(0);
    let escaped!: { set(value: boolean): void };
    await doc.change((tx) => {
      escaped = tx.fields.done;
    });
    expect(() => escaped.set(true)).toThrow("escaped");
    expect(doc.current.done).toBe(false);
  } finally {
    core.free();
  }
});

test("observer failures cannot reject acceptance; save failure is retained until retry", async () => {
  const { core, doc, transport, errors } = await open();
  try {
    const dispose = doc.subscribe(() => {
      throw Error("observer");
    });
    await doc.fields.done.set(true);
    expect(doc.current.done).toBe(true);
    dispose();
    expect(errors.length).toBeGreaterThan(0);
    transport.flush = async () => {
      throw Error("disk unavailable");
    };
    await expect(doc.flush()).rejects.toThrow("disk unavailable");
    expect(doc.status).toBe("save-failed");
    expect(doc.current.done).toBe(true);
    transport.flush = async () => {};
    await doc.flush();
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});

test("a delayed resync cannot roll back later publications", async () => {
  let finish!: (frame: any) => void;
  const p = projection(
    { session: "s", sequence: 1, version: "v1", value: { done: false }, issues: [] },
    () => new Promise((resolve) => (finish = resolve)),
  );
  const gap = p.accept({
    version: "v3",
    patch: { session: "s", previous: 2, sequence: 3, ops: [], issues: [] },
  });
  await p.accept({
    version: "v2",
    patch: {
      session: "s",
      previous: 1,
      sequence: 2,
      ops: [{ type: "set", path: ["done"], value: true }],
      issues: [],
    },
  });
  finish({ session: "s", sequence: 1, version: "v1", value: { done: false }, issues: [] });
  await gap;
  expect(p.get().value.done).toBe(true);
  expect(p.get().sequence).toBe(3);
});

// Gap: close previously admitted new edits while waiting for an attachment blob.
// The blob's reference must finish; unrelated writes must be refused immediately.
test("close stops admission while draining an already admitted attachment commit", async () => {
  const { core, doc } = await open();
  let finish!: () => void;
  const blob = new Promise<void>((resolve) => {
    finish = resolve;
  });
  try {
    const attachment = doc.stageSave(async (commit) => {
      await blob;
      await commit(() => doc.fields.done.set(true));
    });
    const closing = doc.prepareClose();
    await expect(doc.fields.hits.increment()).rejects.toThrow("barrier");
    finish();
    await Promise.all([attachment, closing]);
    expect(doc.current.done).toBe(true);
    expect(doc.current.hits).toBe(0);
    expect(doc.status).toBe("saved");
    doc.cancelClose();
    await doc.fields.hits.increment();
    expect(doc.current.hits).toBe(1);
  } finally {
    finish();
    core.free();
  }
});

// Gap: the core's deleted-row test cannot prove disposal of a composing DOM draft.
// Removing its row must disable the field and allow close without resurrecting it.
test("a deleted focused row retires its composing draft", async () => {
  const { core, doc } = await open();
  const field = new Field();
  const binding = doc.bindText(
    field as unknown as HTMLInputElement,
    doc.fields.rows.item("a").text,
  );
  try {
    field.dispatchEvent(new Event("compositionstart"));
    field.value = "unfinished";
    field.dispatchEvent(new Event("input"));
    await doc.fields.rows.remove("a");
    await doc.prepareClose();
    expect(doc.current.rows.map((row) => row.$id)).toEqual(["b"]);
    expect((field as any).disabled).toBe(true);
  } finally {
    binding.destroy();
    core.free();
  }
});

// Gap: core ancestry tests do not cover more DOM input arriving before a bridge
// acknowledgement. Literal text/selection expectations apply at every latency.
for (const delay of [0, 20, 100, 500]) {
  test(`text binding retains newer input across a ${delay}ms owner reply`, async () => {
    const { core, doc, transport } = await open();
    const send = transport.text;
    transport.text = async request => { if (delay) await Bun.sleep(delay); return send(request); };
    const field = new Field();
    const binding = doc.bindText(field as unknown as HTMLInputElement, doc.fields.title);
    try {
      field.value = "Hello 日本😀";
      field.setSelectionRange(field.value.length, field.value.length);
      field.dispatchEvent(new Event("input"));
      // Yield admission of the first packet, then type before its reply resolves.
      await Promise.resolve();
      field.value += "!";
      field.setSelectionRange(field.value.length, field.value.length);
      field.dispatchEvent(new Event("input"));
      await doc.flush();
      expect(doc.current.title).toBe("Hello 日本😀!");
      expect(field.value).toBe("Hello 日本😀!");
      expect(field.selectionStart).toBe(field.value.length);
    } finally { binding.destroy(); core.free(); }
  });
}

// Gap (moved from the retired engine suite): an attachment reference must never be
// written before its blob is stored, and limits apply before anything is stored.
test("attachment import stores the blob before its reference and deduplicates by content", async () => {
  const { ownerAttachments } = await import("../src/owner/attachments");
  const { core, doc } = await open();
  try {
    const attachments = ownerAttachments(doc, false);
    let storedWhenCommitted = -1;
    const file = new File([new Uint8Array([1, 2, 3])], "a.bin", { type: "application/octet-stream" });
    const ref = await attachments.import(file, {
      async commit(ref) {
        storedWhenCommitted = (await attachments.list()).length;
        await doc.fields.title.replace(ref.id);
      },
    });
    expect(storedWhenCommitted).toBe(1);
    expect(doc.current.title).toBe(ref.id);
    const again = await attachments.import(new File([new Uint8Array([1, 2, 3])], "b.bin"), { commit() {} });
    expect(again.id).toBe(ref.id);
    expect((await attachments.list()).length).toBe(1);
    const huge = new File([new Uint8Array(10 * 1024 * 1024 + 1)], "big.bin");
    await expect(attachments.import(huge, { commit() {} })).rejects.toThrow();
    expect((await attachments.list()).length).toBe(1);
  } finally {
    core.free();
  }
});

// Gap (moved from the retired lifecycle suite): reloading the interface must remount
// against the same document without losing flushed edits.
test("view reload remounts against the same document and keeps flushed edits", async () => {
  const { mountViewLifecycle } = await import("../src/view-lifecycle");
  const { core, doc } = await open();
  try {
    let mounts = 0;
    let recovered = 0;
    const target = { ownerDocument: new EventTarget(), inert: false } as unknown as HTMLElement;
    const handle = await mountViewLifecycle({
      mount: async () => {
        mounts++;
        return { rendered: () => {}, unmount: () => {} };
      },
      document: doc,
      target,
      session: {
        flush: () => doc.flush(),
        handle: async () => ({ ok: true }) as any,
        prepareClose: () => doc.prepareClose(),
        cancelClose: () => doc.cancelClose(),
        close: () => doc.close(),
        discardPending: async () => {},
      },
      capture: { begin: async () => ({}) as any, restore: async () => ({}) as any },
      recovered: async () => {
        recovered++;
      },
    });
    await doc.fields.title.replace("Reloaded");
    await handle.reloadInterface();
    expect(mounts).toBe(2);
    expect(recovered).toBe(1);
    expect(doc.current.title).toBe("Reloaded");
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});
