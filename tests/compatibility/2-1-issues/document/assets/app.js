// Hand-written ABI 1 consumer, independent of the Svelte adapter. It relies only on
// the documented ctx (packages/document/src/abi.ts). Sealed copies of this file in
// tests/compatibility must keep working against every future host.
//
// mount() checks relative behavior only, so it runs against any stored state,
// including anomalous merged state. Any failed check fails the mount, and with it
// the native open or export.
const passed = [];
function check(name, ok) {
  if (!ok) throw new Error(`ABI check failed: ${name}`);
  passed.push(name);
}
function rejects(name, work) {
  try {
    work();
  } catch (error) {
    check(name, error && error.code === "hitslop_operation_rejected");
    return;
  }
  check(name, false);
}

async function exercise(ctx, target) {
  check("abi version", ctx.abi === 1 && Array.isArray(ctx.capabilities));
  const doc = ctx.document;
  check(
    "document shape",
    typeof doc.key === "string" &&
      typeof doc.id === "string" &&
      ["saved", "saving", "save-failed"].includes(doc.status) &&
      Array.isArray(doc.issues) &&
      doc.full === false,
  );

  const events = [];
  const stop = doc.subscribe((event) => events.push(event));

  // Text and rich text.
  const title = doc.current.title;
  doc.fields.title.replace(title + " edited");
  check("text replace", doc.current.title === title + " edited");
  check("local change event", events.some((e) => e.kind === "change" && e.origin === "local"));
  doc.fields.title.splice(0, 0, "¶ ");
  check("text splice", doc.current.title.startsWith("¶ "));
  doc.fields.notes.replace("bold 🌻");
  doc.fields.notes.mark({ start: 0, end: 4 }, "bold", true);
  check("rich text mark", doc.current.notes.delta[0].attributes?.bold === true);
  doc.fields.notes.unmark({ start: 0, end: 4 }, "bold");
  check("rich text unmark", !doc.current.notes.delta.some((span) => span.attributes?.bold));

  // Scalars, counter and validation.
  if (doc.current.count === null) {
    // A merged non-finite counter reads as null and refuses further increments.
    rejects("non-finite counter refuses edits", () => doc.fields.count.increment(1));
  } else {
    const count = doc.current.count;
    doc.fields.count.increment(3);
    doc.fields.count.decrement();
    check("counter", doc.current.count === count + 2);
  }
  doc.fields.done.set(!doc.current.done);
  doc.fields.level.set(7);
  doc.fields.mode.set("b");
  check("scalars", doc.current.level === 7 && doc.current.mode === "b");
  rejects("bounds rejected", () => doc.fields.level.set(99));
  rejects("enum rejected", () => doc.fields.mode.set("z"));
  doc.fields.level.preview(8);
  check("preview", doc.current.level === 8);

  // Scalar lists.
  doc.fields.tags.replace(["a", "b"]);
  doc.fields.tags.insert("c");
  doc.fields.tags.set(0, "A");
  doc.fields.tags.move(2, 0);
  doc.fields.tags.remove(1);
  check("scalar list", JSON.stringify(doc.current.tags) === '["c","b"]');

  // Object rows: identity, move, same-transaction insert-then-address, at().
  const first = doc.fields.rows.insert({ name: "first", done: false }).id;
  const second = doc.fields.rows.insert({ name: "second", done: false }).id;
  check("row ids", typeof first === "string" && first !== second);
  doc.fields.rows.move(first, { after: second });
  const rows = doc.current.rows;
  check("row move keeps identity", rows.at(-1).$id === first && rows.at(-2).$id === second);
  doc.at(rows.at(-1)).done.set(true);
  check("at()", doc.current.rows.at(-1).done === true);
  const inserted = doc.change((tx) => {
    const { id } = tx.fields.rows.insert({ name: "tx", done: false });
    tx.fields.rows.item(id).done.set(true);
    return id;
  });
  check(
    "insert then address in one change",
    doc.current.rows.some((row) => row.$id === inserted && row.done),
  );
  const unchanged = doc.current;
  try {
    doc.change((tx) => {
      tx.fields.title.replace("never");
      throw new Error("abort");
    });
  } catch {}
  check("change is atomic", doc.current === unchanged);
  doc.fields.rows.remove(inserted);
  check("row remove", !doc.current.rows.some((row) => row.$id === inserted));

  // Records, trees and optional composites.
  doc.fields.cells.put("A1", "one");
  doc.fields.cells.entry("A1").set("uno");
  check("record", doc.current.cells.A1 === "uno");
  doc.fields.cells.delete("A1");
  check("record delete", !("A1" in doc.current.cells));
  const root = doc.fields.outline.insert({ label: "root" }).id;
  const child = doc.fields.outline.insert({ label: "child" }, { parent: root }).id;
  doc.fields.outline.move(child, { parent: null });
  check("tree", doc.current.outline.some((node) => node.$id === child));
  doc.fields.outline.remove(root);
  doc.fields.outline.remove(child);
  doc.fields.cover.set({ caption: "cover" });
  check("optional set", doc.current.cover?.caption === "cover");
  doc.fields.cover.clear();
  check("optional clear", doc.current.cover === undefined);

  // Bindings.
  const input = document.createElement("input");
  const box = document.createElement("input");
  box.type = "checkbox";
  target.append(input, box);
  const text = ctx.bind.text(input, doc.fields.title);
  check("bind text renders", input.value === doc.current.title);
  input.value = doc.current.title + "!";
  input.dispatchEvent(new Event("input"));
  await doc.flush();
  check("bind text writes", doc.current.title.endsWith("!"));
  const value = ctx.bind.value(box, doc.fields.done);
  check("bind value renders", box.checked === doc.current.done);
  box.checked = !box.checked;
  box.dispatchEvent(new Event("change"));
  check("bind value writes", doc.current.done === box.checked);
  text.destroy();
  value.destroy();

  // Host services.
  check("theme", typeof ctx.theme.get().effective.accent === "string");
  check("attachments", Array.isArray(await ctx.attachments.list()));
  check("capture", typeof ctx.capture.isRenderer() === "boolean");
  const unregister = ctx.capture.onPrepare(() => {});
  check("capture prepare", typeof unregister === "function");
  unregister();
  check("host calls", typeof ctx.window.resize === "function" && typeof ctx.reportError === "function");

  await doc.flush();
  check("flushed", doc.status === "saved");
  stop();
  const eventCount = events.length;
  doc.fields.title.replace(doc.current.title);
  check("unsubscribe", events.length === eventCount);
}

export default {
  async mount(ctx, target) {
    await exercise(ctx, target);
    const report = document.createElement("main");
    report.dataset.hitslopRoot = "";
    report.style.cssText = "padding:24px;font:16px system-ui;color:var(--slop-accent)";
    report.textContent = `ABI 1: ${passed.length} checks passed`;
    target.append(report);
    return { unmount: () => target.replaceChildren() };
  },
};
