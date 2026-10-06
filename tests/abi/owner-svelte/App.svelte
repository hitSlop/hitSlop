<script lang="ts">
  import { tick, onMount } from "svelte";
  import { attachments, bindText, capture, resizeWindow } from "@hitslop/document/svelte";
  import { isDocumentError, isRejected } from "@hitslop/document";
  import doc from "./schema";
  let shown: HTMLParagraphElement;
  let input: HTMLTextAreaElement;
  let checkbox: HTMLInputElement;
  const check = (passed: boolean, failure: string) => {
    if (!passed) throw new Error(failure);
  };
  const refused = async (write: Promise<unknown>, reason: string) => {
    try {
      await write;
    } catch (error) {
      check(isRejected(error) && error.reason === reason, `Expected a ${reason} refusal, got ${String(error)}`);
      return;
    }
    throw new Error(`Expected a ${reason} refusal`);
  };
  onMount(() => {
    const stopPrepare = capture.onPrepare((mode, signal) => {
      check(["preview", "export", "icon"].includes(mode), "Unknown capture mode");
      check(signal instanceof AbortSignal && !signal.aborted, "Invalid capture signal");
    });
    // Exercises every ctx member and descriptor kind through the public SDK. It works on
    // a document in any prior state, so a frozen copy of this build can run it against
    // later shells (tests/compat).
    (globalThis as any).contractTest = async () => {
      // Text: an accepted write renders; a binding holds a composition, then sends it.
      await doc.fields.title.set("Accepted 😀");
      check(doc.current.title === "Accepted 😀", "Acceptance did not publish");
      await tick();
      check(shown.textContent === "Accepted 😀", "Svelte did not render publication");
      input.value = "Draft 中文";
      input.dispatchEvent(new CompositionEvent("compositionstart"));
      input.dispatchEvent(new Event("input"));
      check(doc.current.title === "Accepted 😀", "Composition sent early");
      input.dispatchEvent(new CompositionEvent("compositionend"));
      await doc.flush();
      check(doc.current.title === "Draft 中文", "Draft was lost");
      // Bound scalars show at once and follow the document.
      checkbox.checked = true;
      checkbox.dispatchEvent(new Event("change"));
      check(doc.current.done, "Bound value did not show at once");
      await doc.flush();
      check(doc.current.done, "Boolean binding lost edit");
      await doc.fields.done.set(false);
      await tick();
      check(!checkbox.checked, "Bound value did not follow the document");
      // Scalars, bounds and refusals.
      await doc.fields.label.set("Label ✓");
      await doc.fields.ratio.set(0.25);
      await doc.fields.count.set(42);
      await doc.fields.lane.set("doing");
      await refused(doc.fields.count.set(1000), "out_of_range");
      await refused(doc.fields.label.set("x".repeat(41)), "out_of_range");
      check(doc.current.count === 42 && doc.current.label === "Label ✓", "A refusal changed the document");
      // Previews and assigned values commit at the next flush.
      doc.fields.ratio.preview(0.75);
      doc.fields.settings.volume.value = 7;
      doc.fields.settings.muted.value = true;
      await doc.flush();
      check(doc.current.ratio === 0.75 && doc.current.settings.volume === 7 && doc.current.settings.muted, "Previews were not committed");
      // Optional scalars, text and objects.
      await doc.fields.note.set("Optional note");
      await doc.fields.note.clear();
      check(doc.current.note === undefined, "Clear left the optional set");
      await doc.fields.memo.set("Memo ✎");
      await doc.fields.place.set({ name: "Café", visits: 1 });
      await doc.fields.place.visits.set(2);
      check(doc.current.place?.visits === 2 && doc.current.memo === "Memo ✎", "Optional object or text was lost");
      // Rows: insert, move, nested scalar lists and records, `at`, remove.
      const first = (await doc.fields.rows.insert({ text: "First", done: false, tags: [], notes: {} })).id;
      const second = (await doc.fields.rows.insert({ text: "Second", done: false, tags: [], notes: {} }, { before: first })).id;
      await doc.fields.rows.move(second, { after: first });
      const row = doc.fields.rows.item(first);
      await row.text.set("First ✓");
      await row.tags.insert("a");
      await row.tags.insert("b", 0);
      await row.tags.set(1, "c");
      await row.tags.replace(["x", "y", "z"]);
      await row.tags.remove(0);
      await row.notes.put("k", "v");
      await row.notes.entry("k").set("w");
      await row.notes.put("gone", "soon");
      await row.notes.delete("gone");
      const shownRow = doc.current.rows.find((r) => r.$id === first)!;
      await doc.at(shownRow).done.set(true);
      await doc.fields.rows.remove(second);
      const kept = doc.current.rows.find((r) => r.$id === first);
      check(
        kept?.text === "First ✓" && kept.done && kept.tags.join() === "y,z" && kept.notes.k === "w" && !("gone" in kept.notes),
        "Row edits were lost",
      );
      check(!doc.current.rows.some((r) => r.$id === second), "Removed row survived");
      // Scalar lists, including an element preview.
      await doc.fields.colors.replace([]);
      await doc.fields.colors.insert("#ffffff");
      await doc.fields.colors.insert("#000000", 0);
      await doc.fields.colors.set(1, "#eeeeee");
      doc.fields.colors.preview(0, "#111111");
      await doc.flush();
      await doc.fields.colors.insert("#222222");
      await doc.fields.colors.remove(2);
      check(doc.current.colors.join() === "#111111,#eeeeee", "Scalar list edits were lost");
      // Records of scalars and of objects.
      await doc.fields.checkins.put("2026-10-02", 1);
      await doc.fields.checkins.entry("2026-10-02").set(2);
      await doc.fields.cells.put("A1", { input: "42" });
      await doc.fields.cells.entry("A1").input.set("43");
      await doc.fields.cells.entry("A1").width.set(10);
      await refused(doc.fields.cells.put("A1", { input: "replace" }), "exists");
      check(doc.current.checkins["2026-10-02"] === 2 && doc.current.cells.A1?.input === "43", "Record edits were lost");
      // Counters add up; a change applies all of its writes or none.
      const hits = doc.current.hits ?? 0;
      await doc.fields.hits.increment(5);
      await doc.fields.hits.increment(-2);
      check(doc.current.hits === hits + 3, "Counter lost an increment");
      const id = await doc.change((tx) => {
        const { id } = tx.fields.rows.insert({ text: "Added", done: false, tags: [], notes: {} });
        tx.fields.rows.item(id).done.set(true);
        return id;
      });
      check(!!doc.current.rows.find((r) => r.$id === id)?.done, "Collector failed");
      await refused(doc.change((tx) => {
        tx.fields.count.set(5);
        tx.fields.count.set(1000);
      }), "out_of_range");
      check(doc.current.count === 42, "A refused change applied part of itself");
      // Undo and redo step through the person's edits.
      await doc.fields.label.set("Before undo");
      await doc.fields.label.set("Undo me");
      await doc.undo();
      check(doc.current.label === "Before undo", "Undo did not revert");
      await doc.redo();
      check(doc.current.label === "Undo me", "Redo did not reapply");
      check(Array.isArray(doc.current.rows) && Array.isArray(doc.issues), "Snapshot or issues missing");
      // Host services: attachments, capture and the window.
      const ref = await attachments.import<typeof doc.descriptor>(new File(["conformance"], "note.txt", { type: "text/plain" }), (tx, ref) =>
        tx.fields.attachment.set(ref.id),
      );
      check(doc.current.attachment === ref.id && ref.name === "note.txt" && ref.mimeType === "text/plain", "Attachment reference was lost");
      check((await (await attachments.read(ref.id, { type: ref.mimeType })).text()) === "conformance", "Attachment bytes differ");
      check(capture.isRenderer() === false, "An interactive page reported itself as a renderer");
      await resizeWindow({ width: 480, height: 480 }).catch((error) => {
        if (!isDocumentError(error)) throw error;
      });
      // A resolved flush means every accepted edit is saved.
      await doc.flush();
      return true;
    };
    return () => {
      stopPrepare();
      delete (globalThis as any).contractTest;
    };
  });
</script>

<main>
  <p bind:this={shown}>{doc.current.title}</p>
  <textarea aria-label="Title" bind:this={input} use:bindText={doc.fields.title}></textarea>
  <input aria-label="Done" type="checkbox" bind:this={checkbox} bind:checked={doc.fields.done.value} />
  <ul>
    {#each doc.current.rows as row (row.$id)}<li>{row.text}</li>{/each}
  </ul>
</main>
