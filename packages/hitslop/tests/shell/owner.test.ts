// Gap: Rust tests cannot prove renderer promises, collector lifetime, stream recovery or
// immutable snapshots. Oracle: literal authored outcomes through the real WASM binding,
// with replies resolving before their publications arrive, as they can natively.
import { expect, test } from "bun:test";
import { OwnerDocument } from "../../src/shell/owner/document";
import type { OwnerTransport } from "../../src/shell/owner/transport";
import { wasmTransport } from "./wasm-transport";
import { defineDocument, s } from "hitslop";
import { DocumentError } from "../../src/sdk/errors";
import type { Batch } from "../../src/schema/core";
import type { PagePush } from "../../src/wire/page";
/** Wraps the transport's text edits (batches whose set carries a selection); other
 * batches pass straight through. */
function interceptText(
  transport: OwnerTransport,
  wrap: (send: OwnerTransport["apply"]) => OwnerTransport["apply"],
) {
  const apply = transport.apply;
  const text = wrap(apply);
  const typed = (batch: Batch) => batch.intents.some((op) => op.type === "set" && op.selection !== undefined);
  transport.apply = (batch) => (typed(batch) ? text(batch) : apply(batch));
}
const moduleURL = new URL("../../../../generated/core/wasm/hitslop_core_wasm.js", import.meta.url);
const wasm = await import(moduleURL.href);
wasm.initSync({
  module: await Bun.file(
    new URL("../../../../generated/core/wasm/hitslop_core_wasm_bg.wasm", import.meta.url),
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

test("WASM binding executes literal core fixtures", async () => {
  for (const name of ["checklist", "scalars", "collections"]) {
  const fixture = await Bun.file(
    new URL(`../../../../crates/hitslop-core/fixtures/${name}.json`, import.meta.url),
  ).json();
  for (const scenario of fixture.scenarios) {
    const core = wasm.WasmDocument.create(
      JSON.stringify(fixture.schema),
      JSON.stringify(scenario.initial ?? fixture.initial),
    );
    try {
      const before = core.state();
      const batch = JSON.stringify({ intents: scenario.intents });
      if (scenario.error) {
        let failure: unknown;
        try { core.applyBatch(batch); } catch (error) { failure = error; }
        expect(failure).toMatchObject({ code: scenario.error });
        await expect(wasmTransport(core).apply({ intents: scenario.intents })).rejects.toMatchObject({
          name: "DocumentError", reason: scenario.error,
        });
        expect(core.state()).toBe(before);
      } else {
        const applied = core.applyBatch(batch);
        // A batch that changes nothing publishes nothing.
        if (applied.publication === undefined) expect(core.state()).toBe(before);
        expect(JSON.parse(core.state()).value).toEqual(scenario.after);
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
  // The document's own push receiver, so a test can deliver what the host would.
  let receiver!: (pushes: PagePush[]) => void;
  const listen = transport.onPush.bind(transport);
  transport.onPush = (next: (pushes: PagePush[]) => void) => { receiver = next; listen(next); };
  const doc = await OwnerDocument.open(definition, transport, (error) => errors.push(error));
  transport.onPush = listen;
  return { core, transport, doc, errors, push: (pushes: PagePush[]) => receiver(pushes) };
}
const gate = () => {
  let release!: () => void;
  const promise = new Promise<void>((resolve) => (release = resolve));
  return { promise, release };
};

test("commands drain pending edits and resolve after the owner's publication", async () => {
  const { core, transport, doc } = await open();
  const held = gate();
  let observed: unknown;
  transport.runCommand = async (name, args) => {
    observed = { name, args, title: JSON.parse(core.state()).value.title };
    await held.promise;
    const result = await transport.apply({ intents: [{ type: "increment", path: ["hits"], by: 3 }] });
    return { sequence: result.sequence, ids: result.ids, result: "accepted" };
  };
  try {
    const edit = doc.fields.title.set("Before command");
    const command = doc.runCommand("increment", { by: 3 });
    await edit;
    await Bun.sleep(0);
    expect(observed).toEqual({ name: "increment", args: { by: 3 }, title: "Before command" });
    expect(doc.current.hits).toBe(0);
    held.release();
    expect(await command).toBe("accepted");
    expect(doc.current.hits).toBe(3);
    await doc.undo();
    expect(doc.current.hits).toBe(0);
    expect(doc.current.title).toBe("Before command");
  } finally { held.release(); core.free(); }
});

test("the page never retries command failures", async () => {
  for (const outcome of ["unknown_outcome", "rejected"] as const) {
    const { core, transport, doc } = await open();
    let calls = 0;
    transport.runCommand = async () => { calls++; throw new DocumentError(outcome, "test outcome", outcome === "rejected" ? "stale_base" : undefined); };
    try {
      await expect(doc.runCommand("increment", {})).rejects.toMatchObject({ code: outcome });
      expect(calls).toBe(1);
      expect(doc.current.hits).toBe(0);
    } finally { core.free(); }
  }
});

// Scalar writes show at once; other writes appear with their publication.
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
    const counted = doc.fields.hits.increment();
    await entered.promise;
    expect(doc.current.rows[0]!.done).toBe(true);
    expect(doc.current.hits).toBe(0);
    expect(JSON.parse(core.state()).value.rows[0].done).toBe(false);
    hold.release();
    await pending;
    await counted;
    expect(doc.current.hits).toBe(1);
    expect(doc.current.rows[0]!.done).toBe(true);
    expect(doc.current.rows[1]).toBe(before.rows[1]);
    expect(before.rows[0]!.done).toBe(false);
    expect(Object.isFrozen(doc.current.rows[0])).toBe(true);
    await doc.flush();
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
      tx.fields.hits.increment(-1);
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
    // A transaction cannot leave a local preview for a later flush to commit.
    expect("preview" in escaped).toBe(false);
    expect(() => escaped.set(true)).toThrow("escaped");
    expect(doc.current.done).toBe(false);
  } finally {
    core.free();
  }
});

test("observer failures cannot reject acceptance; a failed save rejects flush until a retry succeeds", async () => {
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
    expect(doc.current.done).toBe(true);
    transport.flush = flush;
    await doc.flush();
  } finally {
    core.free();
  }
});

// Spike S-D. Failure: a gap in the push stream silently skipped a change, or recovery
// rebuilt the page and lost text the user was still typing. Oracle: the resynced state
// equals the owner's, and the DOM keeps its unsent text and sends it afterwards.
test("a push gap resyncs from a fresh snapshot and keeps unsent text", async () => {
  const { core, transport, doc, push } = await open();
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
    deliver = push;
    input.dispatchEvent(new Event("compositionstart"));
    input.type("Hello there");
    await doc.fields.done.set(true); // arrives with previous = 1 while the page is at 0
    expect(doc.current.hits).toBe(2);
    expect(doc.current.done).toBe(true);
    expect(input.value).toBe("Hello there");
    input.dispatchEvent(new Event("compositionend"));
    await doc.flush();
    expect(doc.current.title).toBe("Hello there");
    expect(JSON.parse(core.state()).value.title).toBe("Hello there");
  } finally {
    binding.destroy();
    core.free();
  }
});

test("a publication older than the current state is ignored", async () => {
  const { core, doc, push } = await open();
  try {
    await doc.fields.hits.increment(1);
    const current = doc.current;
    push([
      {
        type: "publication",
        publication: { previous: 0, sequence: 1, version: "old", ops: [{ type: "set", path: ["hits"], value: 99 }] },
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
    interceptText(transport, (send) => async (batch) => {
      if (delay) await Bun.sleep(delay);
      return send(batch);
    });
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
test("a concurrent whole-field set and page typing survive and undo separately", async () => {
  const { core, transport, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  const hold = gate();
  interceptText(transport, (send) => async (batch) => {
    await hold.promise;
    return send(batch);
  });
  try {
    input.type("Hello!");
    await doc.fields.title.set("Oh Hello"); // lands while the page's request waits
    hold.release();
    await doc.flush();
    expect(doc.current.title).toBe("Oh Hello!");
    expect(input.value).toBe("Oh Hello!");
    await doc.undo();
    expect(doc.current.title).toBe("Oh Hello");
    expect(input.value).toBe("Oh Hello");
    await doc.undo();
    expect(input.value).toBe("Hello");
    await doc.redo();
    expect(input.value).toBe("Oh Hello");
    await doc.redo();
    expect(doc.current.title).toBe("Oh Hello!");
    expect(input.value).toBe("Oh Hello!");
  } finally {
    hold.release();
    binding.destroy();
    core.free();
  }
});

// The page must wait for the host's stored blob before submitting its reference.
test("attachment import uses host identity and media type before writing its reference", async () => {
  const { ownerAttachments } = await import("../../src/shell/attachments");
  const { preview } = await import("../../src/shell/preview");
  const { core, doc } = await open();
  const previous = preview.host;
  const held = gate();
  const id = "a".repeat(64);
  let uploads = 0;
  preview.host = {
    uiURL: "",
    onPush: () => {},
    request: async (request: any) => {
      expect(request).toEqual({ method: "attachments.put", bytes: "AQID" });
      uploads++;
      await held.promise;
      return JSON.stringify({ ok: true, method: "attachments.put", id, byteLength: 3, mimeType: "application/octet-stream" });
    },
    attachmentURL: (key: string) => `http://localhost/attachments/${key}`,
  };
  try {
    const attachments = ownerAttachments(doc);
    const importing = attachments.import(new File([new Uint8Array([1, 2, 3])], "a.bin", { type: "text/html" }), (tx: any, ref) => tx.fields.title.set(ref.id));
    await Bun.sleep(0);
    expect(doc.current.title).toBe("Hello");
    held.release();
    const ref = await importing;
    expect(ref).toEqual({ id, name: "a.bin", byteLength: 3, mimeType: "application/octet-stream" });
    expect(doc.current.title).toBe(id);
    expect(attachments.url(id)).toBe(`http://localhost/attachments/${id}`);
    expect(() => attachments.url("../ui.js")).toThrow();
    await expect(attachments.import(new File([new Uint8Array(10 * 1024 * 1024 + 1)], "big.bin"), () => {})).rejects.toThrow();
    expect(uploads).toBe(1);
  } finally { held.release(); preview.host = previous; core.free(); }
});

// Gap: reloading the interface must remount against the same document without losing
// flushed edits.
test("view reload remounts against the same document and keeps flushed edits", async () => {
  const { mountViewLifecycle } = await import("../../src/shell/view-lifecycle");
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
      recovered: async () => {
        recovered++;
      },
    });
    await doc.fields.title.set("Reloaded");
    await handle.reloadInterface();
    expect(mounts).toBe(2);
    expect(recovered).toBe(1);
    expect(doc.current.title).toBe("Reloaded");
  } finally {
    core.free();
  }
});

// Regressions: detached/retargeted controls must drain their own last value, and an
// unset optional is editable only while its containing row still exists.
for (const retarget of [false, true]) {
  test(`${retarget ? "retarget" : "destroy"} drains newer text behind an in-flight reply`, async () => {
    const { core, doc, transport, errors } = await open();
    const input = field();
    const binding = doc.bindText(input, doc.fields.rows.item("a").text);
    const held = gate(), entered = gate();
    let first = true;
    interceptText(transport, (send) => async (batch) => {
      if (first) { first = false; entered.release(); await held.promise; }
      return send(batch);
    });
    try {
      input.type("First!");
      await entered.promise;
      input.type("First!!");
      if (retarget) {
        binding.update(doc.fields.rows.item("b").text);
        input.type("Second!");
      } else binding.destroy();
      held.release();
      await doc.flush();
      expect(doc.current.rows[0]!.text).toBe("First!!");
      expect(doc.current.rows[1]!.text).toBe(retarget ? "Second!" : "Second");
      expect(errors).toEqual([]);
    } finally { held.release(); binding.destroy(); core.free(); }
  });
}

test("an optional text binding disables when its parent row disappears", async () => {
  const def = defineDocument({ rows: s.list(s.object({ text: s.optional(s.text()) })) });
  const core = wasm.WasmDocument.create(JSON.stringify(def.descriptor), JSON.stringify({ rows: [{ $id: "a" }] }));
  const transport = wasmTransport(core);
  const errors: unknown[] = [];
  const doc = await OwnerDocument.open(def, transport, error => errors.push(error));
  const text = field();
  const tb = doc.bindText(text, doc.fields.rows.item("a").text);
  try {
    expect(text.disabled).toBe(false);
    await doc.fields.rows.remove("a");
    expect(text.disabled).toBe(true);
    await doc.flush();
    expect(errors).toEqual([]);
  } finally { tb.destroy(); core.free(); }
});

for (const outcome of [false, true]) {
  test(`${outcome ? "outcome failure retains" : "semantic rejection reverts"} a preview at a close barrier`, async () => {
    const { DocumentError } = await import("../../src/sdk/internal");
    const def = defineDocument({ n: s.integer({ max: 10 }), valid: s.integer() });
    const core = wasm.WasmDocument.create(JSON.stringify(def.descriptor), JSON.stringify({ n: 1, valid: 0 }));
    const transport = wasmTransport(core);
    const errors: unknown[] = [];
    const doc = await OwnerDocument.open(def, transport, error => errors.push(error));
    const apply = transport.apply;
    try {
      doc.fields.n.preview(outcome ? 2 : 11);
      doc.fields.valid.preview(3);
      if (outcome) transport.apply = async () => { throw new DocumentError("unknown_outcome", "connection lost"); };
      if (outcome) {
        await expect(doc.prepareClose()).rejects.toThrow("connection lost");
        expect(doc.current.n).toBe(2);
        transport.apply = apply;
        await doc.prepareClose();
        expect(doc.current.n).toBe(2);
      } else {
        await doc.prepareClose();
        expect(doc.current.n).toBe(1);
        expect(errors).toHaveLength(1);
      }
      expect(doc.current.valid).toBe(3);
      expect(JSON.parse(core.state()).value).toEqual({ n: outcome ? 2 : 1, valid: 3 });
    } finally { core.free(); }
  });
}

test("caught ordinary rejections are not reported as unhandled errors", async () => {
  const { core, doc, errors } = await open();
  try {
    await expect(doc.fields.hits.increment(0)).rejects.toThrow();
    await expect(doc.change(() => { throw Error("author handled"); })).rejects.toThrow();
    expect(errors).toEqual([]);
  } finally { core.free(); }
});

// A final lost publication has no later sequence gap to trigger recovery.
test("a lost final publication settles without replaying the accepted mutation", async () => {
  const { core, transport, doc } = await open();
  transport.onPush(() => {});
  try {
    await doc.fields.hits.increment(3);
    expect(doc.current.hits).toBe(3);
    expect(JSON.parse(core.state()).value.hits).toBe(3);
  } finally { core.free(); }
}, 4000);

for (const text of [false, true]) test(`an unknown ${text ? "text" : "scalar"} outcome retains input and fails the barrier`, async () => {
  const { DocumentError } = await import("../../src/sdk/internal");
  const def = defineDocument({ value: text ? s.text() : s.string() });
  const core = wasm.WasmDocument.create(JSON.stringify(def.descriptor), JSON.stringify({ value: "before" }));
  const transport = wasmTransport(core);
  const errors: unknown[] = [];
  const doc = await OwnerDocument.open(def, transport, error => errors.push(error));
  const input = field();
  const binding = text ? doc.bindText(input, doc.fields.value as never) : undefined;
  const refused = async () => { throw new DocumentError("unknown_outcome", "connection lost"); };
  transport.apply = refused;
  try {
    if (text) input.type("unsent draft");
    else (doc.fields.value as unknown as { value: string }).value = "unsent draft";
    await expect(doc.prepareClose()).rejects.toThrow("connection lost");
    if (text) {
      expect(input.value).toBe("unsent draft");
      expect(errors).toHaveLength(1);
    } else expect(doc.current.value).toBe("unsent draft");
  } finally { binding?.destroy(); core.free(); }
});

test("recovery confirms a lost text reply before draining newer input", async () => {
  const { DocumentError } = await import("../../src/sdk/internal");
  const def = defineDocument({ value: s.text() });
  const core = wasm.WasmDocument.create(JSON.stringify(def.descriptor), JSON.stringify({ value: "A" }));
  const transport = wasmTransport(core);
  const doc = await OwnerDocument.open(def, transport, () => {});
  const input = field();
  const binding = doc.bindText(input, doc.fields.value);
  const held = gate(), entered = gate();
  let calls = 0;
  const apply = transport.apply;
  const lost = async <T>(operation: () => Promise<T>) => {
    const result = await operation();
    if (++calls === 1) { entered.release(); await held.promise; throw new DocumentError("unknown_outcome", "lost reply"); }
    return result;
  };
  transport.apply = batch => lost(() => apply(batch));
  try {
    input.type("AB"); await entered.promise;
    input.type("ABC"); held.release();
    await doc.flush();
    expect(doc.current.value).toBe("ABC");
    expect(JSON.parse(core.state()).value.value).toBe("ABC");
    expect(calls).toBe(2);
  } finally { held.release(); binding.destroy(); core.free(); }
});

// Failure: text typed while recovery had failed was refused before it left the page,
// but was recorded as an uncertain outcome that no snapshot could ever confirm, so every
// later flush and close failed. Oracle: after recovery the typed text is saved once.
test("text refused before sending is sent once the document recovers", async () => {
  const { DocumentError } = await import("../../src/sdk/internal");
  const { core, transport, doc, push } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  const reopen = transport.open;
  try {
    transport.open = async () => { throw new DocumentError("owner_invalidated", "owner unavailable"); };
    // The host asks for a resync, and the owner cannot answer it: edits are refused.
    push([{ type: "resync" }]);
    await Bun.sleep(1);
    await expect(doc.fields.done.set(true)).rejects.toThrow("owner unavailable");
    input.type("Hello again");
    await Bun.sleep(1);
    transport.open = reopen;
    await doc.flush();
    expect(doc.current.title).toBe("Hello again");
    expect(JSON.parse(core.state()).value.title).toBe("Hello again");
    await doc.flush();
  } finally { transport.open = reopen; binding.destroy(); core.free(); }
});

// Failure: a destroyed binding whose final draft could not be committed stayed in the
// barrier set, so every later close failed with no control left to fix it.
test("a destroyed text binding with an unresolvable draft does not block close", async () => {
  const { DocumentError } = await import("../../src/sdk/internal");
  const def = defineDocument({ value: s.text() });
  const core = wasm.WasmDocument.create(JSON.stringify(def.descriptor), JSON.stringify({ value: "before" }));
  const transport = wasmTransport(core);
  const errors: unknown[] = [];
  const doc = await OwnerDocument.open(def, transport, error => errors.push(error));
  const input = field();
  const binding = doc.bindText(input, doc.fields.value);
  const refused = async () => { throw new DocumentError("unknown_outcome", "connection lost"); };
  transport.apply = refused;
  try {
    input.type("unsent draft");
    binding.destroy();
    await Bun.sleep(5);
    await doc.prepareClose();
    expect(errors.length).toBeGreaterThan(0);
    expect(JSON.parse(core.state()).value.value).toBe("before");
  } finally { core.free(); }
});

// Failure: only a plain handle set/clear settled a preview, so writes through change(),
// replace() or insert() left stale previews covering the saved value (Pixel Art's undo
// looked ignored) and the next flush wrote them back. A preview beneath an unset
// optional object also invented that object.
test("accepted writes covering a preview clear it, and previews never invent parents", async () => {
  const def = defineDocument({
    pixels: s.list(s.string()),
    note: s.optional(s.object({ text: s.string() })),
  });
  const core = wasm.WasmDocument.create(JSON.stringify(def.descriptor), JSON.stringify({ pixels: ["", "", ""] }));
  const doc = await OwnerDocument.open(def, wasmTransport(core));
  try {
    doc.fields.pixels.preview(1, "red");
    expect(doc.current.pixels[1]).toBe("red");
    await doc.change((tx) => tx.fields.pixels.set(1, "red"));
    await doc.change((tx) => tx.fields.pixels.set(1, ""));
    expect(doc.current.pixels).toEqual(["", "", ""]);
    doc.fields.pixels.preview(2, "blue");
    await doc.fields.pixels.insert("x", 0);
    expect(doc.current.pixels).toEqual(["x", "", "", ""]);
    doc.fields.note.text.preview("draft");
    expect(doc.current.note).toBeUndefined();
    await doc.prepareClose();
    expect(JSON.parse(core.state()).value).toEqual({ pixels: ["x", "", "", ""] });
  } finally { core.free(); }
});

// Edit ▸ Undo. Failure: undo skipped what the person had typed but not yet sent, or a
// field's own undo replayed its history as new typing. Oracle: literal values in the
// field and the document.
test("undo sends unsent text first, then reverts the person's steps in order", async () => {
  const { core, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  try {
    await doc.fields.done.set(true);
    // Still composing, so nothing is sent until undo drains it.
    input.dispatchEvent(new Event("compositionstart"));
    input.type("Hello!");
    await doc.undo();
    expect(doc.current.title).toBe("Hello");
    expect(input.value).toBe("Hello");
    expect(doc.current.done).toBe(true);
    await doc.undo();
    expect(doc.current.done).toBe(false);
    await doc.undo();
    expect(doc.current.done).toBe(false);
    await doc.redo();
    expect(doc.current.done).toBe(true);
    await doc.redo();
    expect(input.value).toBe("Hello!");
  } finally {
    binding.destroy();
    core.free();
  }
});

test("a text field's own undo is the document's undo", async () => {
  const { core, doc } = await open();
  const input = field();
  const binding = doc.bindText(input, doc.fields.title);
  try {
    input.type("Hello there");
    await doc.flush();
    const undo = Object.assign(new Event("beforeinput", { cancelable: true }), { inputType: "historyUndo" });
    input.dispatchEvent(undo);
    expect(undo.defaultPrevented).toBe(true);
    for (let i = 0; i < 100 && doc.current.title !== "Hello"; i++) await Bun.sleep(2);
    expect(doc.current.title).toBe("Hello");
    expect(input.value).toBe("Hello");
  } finally {
    binding.destroy();
    core.free();
  }
});
