// Guards reload without document replacement, save-before-capture, failed close and render recovery.
import { test, expect } from "bun:test";
import { Document } from "../src/document";
import { defineDocument, s } from "../src/schema";
import { MemoryStore } from "../src/memory";
import { Session } from "../src/session";
import { mountViewLifecycle } from "../src/view-lifecycle";
import { bindText } from "../src/bind-text";

const definition = defineDocument({ title: s.text() });

async function setup() {
  const store = new MemoryStore();
  const doc = await Document.open(definition, store, { title: "Initial" });
  const session = new Session(doc, "test");
  const events = new EventTarget();
  const target = { ownerDocument: events, inert: false } as unknown as HTMLElement;
  const log: string[] = [];
  let rendering: () => Promise<void> = async () => {};
  const lifecycle = await mountViewLifecycle({
    document: doc,
    target,
    session,
    mount() {
        log.push("mount");
        return {
          async rendered() {
            log.push("render");
            await rendering();
          },
          unmount() {
            log.push("unmount");
          },
        };
    },
    capture: {
      async begin(token, mode) {
        log.push(`capture:${token}:${mode}`);
        return { x: 0, y: 0, width: 100, height: 100, dedicated: false };
      },
      async restore(token) {
        log.push(`restore:${token}`);
      },
    },
    recovered: async () => {
      log.push("recovered");
    },
  });
  return {
    store,
    doc,
    session,
    events,
    target,
    log,
    lifecycle,
    renderWith(callback: () => Promise<void>) {
      rendering = callback;
    },
  };
}

test("views reload against the same document and retain flushed edits", async () => {
  const h = await setup();
  h.doc.fields.title.replace("Edited");
  await h.lifecycle.reloadInterface();
  expect(h.log.filter((entry) => entry === "mount" || entry === "unmount")).toEqual(["mount", "unmount", "mount"]);
  expect(h.log).toContain("recovered");
  const reply = await h.lifecycle.request({ id: "get", documentPath: "test", method: "get" });
  expect(reply.state).toEqual({ title: "Edited" });
  await h.lifecycle.close();
  const reopened = await Document.open(definition, h.store, { title: "Initial" });
  expect(reopened.current.title).toBe("Edited");
  await reopened.close();
});

test("capture waits for durable edits and completed rendering", async () => {
  const h = await setup();
  const append = h.store.append.bind(h.store);
  h.store.append = async (...args) => {
    h.log.push("save");
    return append(...args);
  };
  let release!: () => void;
  let entered!: () => void;
  const started = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  h.renderWith(async () => {
    entered();
    await gate;
  });
  h.doc.fields.title.replace("For export");
  const capturing = h.lifecycle.captureBegin("token");
  await started;
  expect(h.log).toContain("save");
  expect(h.log).not.toContain("capture:token:export");
  const saved = await Document.open(definition, h.store, { title: "Initial" });
  expect(saved.current.title).toBe("For export");
  await saved.close();
  release();
  await capturing;
  await h.lifecycle.captureRestore("token");
  expect(h.log.slice(-2)).toEqual(["capture:token:export", "restore:token"]);
  await h.lifecycle.close();
});

test("failed saves retain the view; retry, cancel and close preserve ordering", async () => {
  const h = await setup();
  const append = h.store.append.bind(h.store);
  h.store.append = async () => {
    throw new Error("disk full");
  };
  h.doc.fields.title.replace("Pending");
  expect(await h.lifecycle.retrySave()).toBe(false);
  await expect(h.lifecycle.prepareClose()).rejects.toThrow("disk full");
  await expect(h.lifecycle.close()).rejects.toThrow("disk full");
  await expect(h.lifecycle.reloadInterface()).rejects.toThrow("disk full");
  await expect(h.lifecycle.captureBegin("failed")).rejects.toThrow("disk full");
  expect(h.target.inert).toBe(false);
  expect(h.log).not.toContain("unmount");
  expect(h.log).not.toContain("capture:failed:export");
  h.store.append = append;
  expect(await h.lifecycle.retrySave()).toBe(true);
  await h.lifecycle.prepareClose();
  expect(h.target.inert).toBe(true);
  expect(() => h.doc.fields.title.replace("Blocked")).toThrow();
  h.lifecycle.cancelClose();
  expect(h.target.inert).toBe(false);
  h.doc.fields.title.replace("Resumed");
  const close = h.session.close.bind(h.session);
  h.session.close = async () => {
    await close();
    h.log.push("closed");
  };
  await h.lifecycle.close();
  expect(h.log.slice(-2)).toEqual(["closed", "unmount"]);
});

test("render failures prevent recovery acknowledgement and a later reload can recover", async () => {
  const h = await setup();
  h.renderWith(async () => {
    h.events.dispatchEvent(
      new CustomEvent("hitslop:render-error", { detail: new Error("broken view") }),
    );
  });
  await expect(h.lifecycle.reloadInterface()).rejects.toThrow("broken view");
  expect(h.log).not.toContain("recovered");
  h.renderWith(async () => {});
  await h.lifecycle.reloadInterface();
  expect(h.log).toContain("recovered");
  await h.lifecycle.close();
});

// The old DOM's uncommitted composition must disappear only after durable state
// loads successfully; new bindings must still flush compositions after remount.
test("discard awaits storage, retains a failed view, and remounts editable bindings", async () => {
  const store = new MemoryStore();
  const doc = await Document.open(definition, store, { title: "Saved" });
  const session = new Session(doc, "test");
  const target = { inert: false } as HTMLElement;
  let input!: HTMLInputElement;
  const lifecycle = await mountViewLifecycle({
    document: doc, session, target,
    mount() {
      input = Object.assign(new EventTarget(), { value: "", disabled: false }) as unknown as HTMLInputElement;
      const binding = bindText(input, doc.fields.title);
      return { rendered() {}, unmount: () => binding.destroy() };
    },
    capture: { async begin() { throw new Error("unused"); }, async restore() {} },
  });
  input.dispatchEvent(new Event("compositionstart"));
  input.value = "Unsaved composition";
  const oldInput = input;
  const load = store.load.bind(store);
  store.load = async () => { throw new Error("Cannot load"); };
  await expect(lifecycle.discardPending()).rejects.toThrow("Cannot load");
  expect(input).toBe(oldInput);
  expect(input.value).toBe("Unsaved composition");
  expect(target.inert).toBe(false);
  let release!: () => void;
  let entered!: () => void;
  const started = new Promise<void>(resolve => entered = resolve);
  const gate = new Promise<void>(resolve => release = resolve);
  store.load = async () => { entered(); await gate; return load(); };
  const discarding = lifecycle.discardPending();
  await started;
  expect(input).toBe(oldInput);
  expect(target.inert).toBe(true);
  expect(() => doc.fields.title.replace("racing edit")).toThrow("discarding");
  release();
  await discarding;
  expect(input).not.toBe(oldInput);
  expect(input.value).toBe("Saved");
  expect(target.inert).toBe(false);
  input.dispatchEvent(new Event("compositionstart"));
  input.value = "New composition";
  await lifecycle.close();
  const reopened = await Document.open(definition, store, { title: "Unused" });
  expect(reopened.current.title).toBe("New composition");
  await reopened.close();
});
