// Gap: Rust tests cannot prove renderer promises, collector lifetime, stream recovery or
// immutable snapshots. Oracle: literal authored outcomes through the real WASM binding,
// with replies resolving before their publications arrive, as they can natively.
import { expect, test } from "bun:test";
import { OwnerDocument } from "../src/owner/document";
import { wasmTransport, type OwnerTransport } from "../src/owner/transport";
import { defineDocument, s } from "../src/schema";
import { Check } from "typebox/value";
import { OwnerStateSchema, OwnerPublicationSchema, type PagePush } from "@hitslop/schema/owner";
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
  for (const name of ["checklist", "scalars"]) {
  const fixture = await Bun.file(
    new URL(`../../../crates/hitslop-core/fixtures/${name}.json`, import.meta.url),
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
      const batch = JSON.stringify({ intents: scenario.intents });
      if (scenario.error) {
        expect(() => core.applyBatch(batch)).toThrow(scenario.error);
        expect(core.snapshot()).toBe(before);
      } else {
        const applied = core.applyBatch(batch);
        expect(Check(OwnerPublicationSchema, JSON.parse(applied.publication))).toBe(true);
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
  }
});

async function open(state = initial) {
  const core = wasm.WasmDocument.create(JSON.stringify(definition.descriptor), JSON.stringify(state));
  const errors: unknown[] = [];
  const transport = wasmTransport(core) as OwnerTransport & Record<string, any>;
  const doc = await OwnerDocument.open(definition, transport, (error) => errors.push(error));
  return { core, transport, doc, errors };
}
const gate = () => {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => (release = resolve));
  return { promise, release };
};

test("ordinary writes resolve after publication and preserve unaffected snapshot identity", async () => {
  const { core, transport, doc } = await open();
  try {
    const hold = gate(), entered = gate();
    const apply = transport.apply;
    transport.apply = async (batch) => {
      entered.release();
      await hold.promise;
      return apply(batch);
    };
    const before = doc.current;
    const pending = doc.fields.rows.item("a").done.set(true);
    await entered.promise;
    expect(doc.current).toBe(before);
    expect(doc.status).toBe("pending");
    hold.release();
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
  disabled = false;
  setSelectionRange(start: number, end: number) {
    this.selectionStart = start;
    this.selectionEnd = end;
  }
  blur() {}
  type(value: string) {
    this.value = value;
    this.setSelectionRange(value.length, value.length);
    this.dispatchEvent(new Event("input"));
  }
}
const field = () => new Field() as Field & HTMLInputElement;

test("text composition stays local until it ends, then flush sends the committed text", async () => {
  const { core, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  try {
    input.dispatchEvent(new Event("compositionstart"));
    input.type("Hello 日本😀");
    await Bun.sleep(5);
    expect(doc.current.title).toBe("Hello");
    input.dispatchEvent(new Event("compositionend"));
    await doc.flush();
    expect(doc.current.title).toBe("Hello 日本😀");
    expect(input.value).toBe("Hello 日本😀");
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
      tx.fields.rows.item(result.id).text.set("New title");
      tx.fields.rows.item(result.id).text.set("Final");
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
    // Handles are cached by path: repeated lookups do not rebuild them.
    expect(doc.at(doc.current.rows[0]!)).toBe(doc.fields.rows.item("a"));
  } finally {
    core.free();
  }
});

test("throwing, async, nested and escaped collectors never submit their staged changes", async () => {
  const { core, doc, transport } = await open();
  try {
    let sent = 0;
    const apply = transport.apply;
    transport.apply = (batch) => {
      sent++;
      return apply(batch);
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
    const flush = transport.flush;
    transport.flush = async () => {
      throw Error("disk unavailable");
    };
    await expect(doc.flush()).rejects.toThrow("disk unavailable");
    expect(doc.status).toBe("save-failed");
    expect(doc.current.done).toBe(true);
    transport.flush = flush;
    await doc.flush();
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});

// Spike S-D. Failure: a gap in the push stream silently skipped a change, or recovery
// rebuilt the page and lost text the user was still typing. Oracle: the resynced state
// equals the owner's, and the DOM keeps its unsent text and sends it afterwards.
test("a push gap resyncs from a fresh snapshot and keeps unsent text", async () => {
  const { core, transport, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  let deliver!: (pushes: PagePush[]) => void;
  transport.onPush((pushes) => deliver(pushes));
  try {
    const dropped: PagePush[] = [];
    deliver = (pushes) => dropped.push(...pushes); // this publication is lost in delivery
    await transport.apply({ intents: [{ type: "increment", path: ["hits"], by: 2 }] });
    await Bun.sleep(5);
    expect(dropped.length).toBe(1);
    deliver = (pushes) => (doc as any).store.publish(pushes);
    input.dispatchEvent(new Event("compositionstart"));
    input.type("Hello there");
    await doc.fields.done.set(true); // arrives with previous = 1 while the page is at 0
    expect(doc.current.hits).toBe(2);
    expect(doc.current.done).toBe(true);
    expect(input.value).toBe("Hello there");
    input.dispatchEvent(new Event("compositionend"));
    await doc.flush();
    expect(doc.current.title).toBe("Hello there");
    expect(JSON.parse(core.snapshot()).value.title).toBe("Hello there");
  } finally {
    binding.destroy();
    core.free();
  }
});

test("a publication older than the current state is ignored", async () => {
  const { core, doc } = await open();
  try {
    await doc.fields.hits.increment(1);
    const current = doc.current;
    (doc as any).store.publish([
      {
        type: "publication",
        publication: { previous: 0, sequence: 1, version: "old", ops: [{ type: "set", path: ["hits"], value: 99 }], issues: [] },
      },
    ]);
    expect(doc.current).toBe(current);
  } finally {
    core.free();
  }
});

// Bug 3. Failure: a close barrier that joined a flush already past its drain point did
// not wait for an attachment import started after it. Oracle: when the barrier resolves,
// the reference is in the document.
test("close waits for an import that started while a flush was running", async () => {
  const { core, doc, transport } = await open();
  const saving = gate(), stored = gate();
  const flush = transport.flush;
  let first = true;
  transport.flush = async () => {
    if (first) {
      first = false;
      await saving.promise;
    }
    return flush();
  };
  try {
    const flushing = doc.flush();
    await Bun.sleep(0);
    const imported = doc.admit(
      async () => {
        await stored.promise;
        return "blob";
      },
      (tx, ref) => tx.fields.title.set(ref),
    );
    const closing = doc.prepareClose();
    saving.release();
    await flushing;
    await expect(doc.fields.hits.increment()).rejects.toThrow("barrier");
    stored.release();
    await closing;
    expect(doc.current.title).toBe("blob");
    await imported;
    expect(doc.current.hits).toBe(0);
    expect(doc.status).toBe("saved");
    doc.cancelClose();
    await doc.fields.hits.increment();
    expect(doc.current.hits).toBe(1);
  } finally {
    saving.release();
    stored.release();
    core.free();
  }
});

// Gap: the core's deleted-row test cannot prove disposal of a composing DOM field.
test("a deleted focused row disables its field and close still completes", async () => {
  const { core, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.rows.item("a").text);
  try {
    input.dispatchEvent(new Event("compositionstart"));
    input.type("unfinished");
    await doc.fields.rows.remove("a");
    await doc.prepareClose();
    expect(doc.current.rows.map((row) => row.$id)).toEqual(["b"]);
    expect(input.disabled).toBe(true);
  } finally {
    binding.destroy();
    core.free();
  }
});

// Failure: switching a binding to another row threw while text was unsent, or lost it.
test("retargeting a binding sends the old field's unsent text first", async () => {
  const { core, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.rows.item("a").text);
  try {
    input.dispatchEvent(new Event("compositionstart"));
    input.type("First edited");
    binding.update(doc.fields.rows.item("b").text);
    await doc.flush();
    expect(doc.current.rows[0]!.text).toBe("First edited");
    expect(input.value).toBe("Second");
  } finally {
    binding.destroy();
    core.free();
  }
});

// Failure: more typing before a reply was lost, duplicated or misplaced the caret.
for (const delay of [0, 20, 100]) {
  test(`text binding retains newer input across a ${delay}ms owner reply`, async () => {
    const { core, doc, transport } = await open();
    const send = transport.text;
    transport.text = async (request) => {
      if (delay) await Bun.sleep(delay);
      return send(request);
    };
    const input = field();
    const binding = doc.bindText(input, doc.fields.title);
    try {
      input.type("Hello 日本😀");
      await Promise.resolve();
      input.type(input.value + "!");
      await doc.flush();
      expect(doc.current.title).toBe("Hello 日本😀!");
      expect(input.value).toBe("Hello 日本😀!");
      expect(input.selectionStart).toBe(input.value.length);
    } finally {
      binding.destroy();
      core.free();
    }
  });
}

// Failure: a CLI or second binding edit to the same field overwrote the user's text.
test("a concurrent whole-field set and page typing both survive", async () => {
  const { core, transport, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  const hold = gate();
  const send = transport.text;
  transport.text = async (request) => {
    await hold.promise;
    return send(request);
  };
  try {
    input.type("Hello!");
    await doc.fields.title.set("Oh Hello"); // lands while the page's request waits
    hold.release();
    await doc.flush();
    expect(doc.current.title).toBe("Oh Hello!");
    expect(input.value).toBe("Oh Hello!");
  } finally {
    hold.release();
    binding.destroy();
    core.free();
  }
});

// Gap: an attachment reference must never be written before its blob is stored, and
// limits apply before anything is stored.
test("attachment import stores the blob before its reference and deduplicates by content", async () => {
  const { ownerAttachments } = await import("../src/owner/attachments");
  const { core, doc } = await open();
  try {
    const attachments = ownerAttachments(doc, false);
    const file = new File([new Uint8Array([1, 2, 3])], "a.bin", { type: "application/octet-stream" });
    let storedWhenReferenced = -1;
    const store = attachments.store as any;
    const ref = await attachments.import(file, (tx: any, ref) => {
      storedWhenReferenced = store.files.size;
      tx.fields.title.set(ref.id);
    });
    expect(storedWhenReferenced).toBe(1);
    expect(doc.current.title).toBe(ref.id);
    const again = await attachments.import(new File([new Uint8Array([1, 2, 3])], "b.bin"), () => {});
    expect(again.id).toBe(ref.id);
    expect((await attachments.list()).length).toBe(1);
    const huge = new File([new Uint8Array(10 * 1024 * 1024 + 1)], "big.bin");
    await expect(attachments.import(huge, () => {})).rejects.toThrow();
    expect((await attachments.list()).length).toBe(1);
    // A reference the core refuses rejects the import.
    await expect(
      attachments.import(new File([new Uint8Array([4])], "c.bin"), (tx: any) => tx.fields.rows.remove("missing")),
    ).rejects.toThrow();
  } finally {
    core.free();
  }
});

// Gap: reloading the interface must remount against the same document without losing
// flushed edits.
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
        applyTheme: () => {},
        prepareClose: () => doc.prepareClose(),
        cancelClose: () => doc.cancelClose(),
      },
      recovered: async () => {
        recovered++;
      },
    });
    await doc.fields.title.set("Reloaded");
    await handle.reloadInterface();
    expect(mounts).toBe(2);
    expect(recovered).toBe(1);
    expect(doc.current.title).toBe("Reloaded");
    expect(doc.status).toBe("saved");
  } finally {
    core.free();
  }
});
