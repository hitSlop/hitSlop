<script lang="ts">
import "./styles.css";
  import { tick, onMount } from "svelte";
  import { attachments, bindText, getDocument, resizeWindow, speech, isSpeechError } from "@hitslop/tinys/svelte";
  import { isDocumentError, isRejected, refuse } from "@hitslop/tinys";
  import schema from "./schema";
  // Access the mounted ctx to probe its framework-neutral ABI, beyond the Svelte adapter.
  import type { TinyDocument } from "../../../packages/tinys/src/sdk/abi";
  import { current as mountedContext } from "../../../packages/tinys/src/sdk/app/context";
  import { mediaURLs } from "./media";
  // App media the corpus must keep serving: an image, audio and a font (styles.css).
  import swatch from "./media/swatch.png";
  import tone from "./media/tone.wav";
  const doc = getDocument(schema);
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
      // An IME edit based on stale text merges with a later whole-field write.
      await doc.fields.title.set("base"); await tick();
      input.dispatchEvent(new CompositionEvent("compositionstart")); input.value = "base local"; input.dispatchEvent(new Event("input"));
      await doc.fields.title.set("remote base"); input.dispatchEvent(new CompositionEvent("compositionend")); await doc.flush();
      check(doc.current.title === "remote base local", "Stale composition lost a concurrent edit");
      // A binding may switch fields without retaining the old field's events.
      const control = document.createElement("textarea"); const binding = bindText(control, doc.fields.title);
      await doc.fields.memo.set("Binding target"); binding.update(doc.fields.memo); control.value = "Binding updated"; control.dispatchEvent(new Event("input")); await doc.flush(); binding.destroy();
      check(doc.current.memo === "Binding updated" && doc.current.title === "remote base local", "Binding.update wrote the old field");
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
      // A drawing/slider gesture previews on separate frames but authors one undo step.
      await doc.fields.ratio.set(0.1);
      for (const value of [0.2, 0.4, 0.6]) {
        await new Promise<void>((resolve) => {
          // A windowless native conformance view may suspend animation frames.
          // Visible views use real frames; the fallback still separates event turns.
          const frame = requestAnimationFrame(() => { clearTimeout(timer); resolve(); });
          const timer = setTimeout(() => { cancelAnimationFrame(frame); resolve(); }, 50);
        });
        doc.fields.ratio.preview(value);
        check(doc.current.ratio === value, "Gesture preview did not show immediately");
      }
      await doc.fields.ratio.set(0.8);
      await doc.undo();
      check(doc.current.ratio === 0.1, "Gesture previews created extra undo steps");
      await doc.redo();
      check(doc.current.ratio === 0.8, "Gesture redo did not restore the final value");
      // Optional scalars, text and objects.
      await doc.fields.note.set("Optional note");
      await doc.fields.note.clear();
      check(doc.current.note === undefined, "Clear left the optional set");
      await doc.fields.memo.set("Memo ✎");
      await doc.fields.place.$set({ name: "Café", visits: 1 });
      await doc.fields.place.visits.set(2);
      check(doc.current.place?.visits === 2 && doc.current.memo === "Memo ✎", "Optional object or text was lost");
      await doc.fields.place.$clear(); check(doc.current.place === undefined, "Optional object clear failed");
      await doc.fields.memo.clear(); check(doc.current.memo === undefined, "Optional text clear failed");
      await doc.fields.note.set("Clear by assignment"); doc.fields.note.value = undefined; await doc.flush(); check(doc.current.note === undefined, "Optional scalar assignment did not clear");
      await doc.fields.enabled.set(true); await doc.fields.enabled.clear(); doc.fields.weight.value = 0.5; await doc.flush(); doc.fields.weight.value = undefined; await doc.flush();
      check(doc.current.enabled === undefined && doc.current.weight === undefined, "Optional typed scalars did not clear");
      await doc.fields.stage.set("done"); await doc.fields.stage.clear(); check(doc.current.stage === undefined, "Optional enum clear failed");
      // Rows: insert, move, nested scalar lists and records, `at`, remove.
      const first = (await doc.fields.rows.insert({ text: "First", done: false, tags: [], notes: {} })).id;
      const second = (await doc.fields.rows.insert({ text: "Second", done: false, tags: [], notes: {} }, { before: first })).id;
      await doc.fields.rows.move(second, { after: first });
      const trailing = (await doc.fields.rows.insert({ text: "After", done: false }, { after: first })).id;
      await doc.fields.rows.move(trailing); check(doc.current.rows.at(-1)?.$id === trailing, "Unanchored move did not append"); await doc.fields.rows.remove(trailing);
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
      // Several board submissions start together; each promise names its own durable row.
      const submissions = ["Board A", "Board B", "Board C"];
      const added = await Promise.all(submissions.map((text) =>
        doc.fields.rows.insert({ text, done: false, tags: [], notes: {} }),
      ));
      check(new Set(added.map(({ id }) => id)).size === submissions.length, "Concurrent submissions reused a row ID");
      await doc.fields.rows.move(added[2]!.id, { before: added[0]!.id });
      await Promise.all(added.map(({ id }, index) => doc.fields.rows.item(id).text.set(`${submissions[index]} ✓`)));
      for (const [index, { id }] of added.entries()) {
        check(doc.current.rows.find((row) => row.$id === id)?.text === `${submissions[index]} ✓`, "A board edit reached the wrong row");
      }
      check(doc.current.rows.findIndex((row) => row.$id === added[2]!.id) <
        doc.current.rows.findIndex((row) => row.$id === added[0]!.id), "A board move lost row identity");
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
      await doc.fields.colors.replace(["a", "b", "c"]); await doc.fields.colors.remove(0, 2); check(doc.current.colors.join() === "c", "Multi-element remove failed");
      // Records of scalars and of objects.
      await doc.fields.checkins.put("2026-10-02", 1);
      await doc.fields.checkins.entry("2026-10-02").set(2);
      await doc.fields.cells.put("A1", { input: "42", hits: 0 });
      await doc.fields.cells.entry("A1").input.set("43");
      await doc.fields.cells.entry("A1").width.set(10);
      await doc.fields.cells.put("A1", { input: "replace", hits: 0 });
      check(doc.current.checkins["2026-10-02"] === 2 && doc.current.cells.A1?.input === "replace" &&
        doc.current.cells.A1.width === undefined, "Complete record replacement failed");
      // A keyed journal editor is removed during composition and then recreated at the
      // same key. Late events from the detached control must not edit its successor.
      await tick();
      const oldEntry = document.querySelector<HTMLTextAreaElement>('[aria-label="Keyed entry"]')!;
      check(oldEntry?.value === "replace", "The keyed text binding did not render");
      oldEntry.dispatchEvent(new CompositionEvent("compositionstart"));
      oldEntry.value = "Unfinished old entry";
      oldEntry.dispatchEvent(new Event("input"));
      await doc.fields.cells.delete("A1");
      await tick();
      check(!oldEntry.isConnected, "Deleting a key retained its editor");
      await doc.fields.cells.put("A1", { input: "Fresh entry", hits: 0 });
      await tick();
      const newEntry = document.querySelector<HTMLTextAreaElement>('[aria-label="Keyed entry"]')!;
      check(newEntry !== oldEntry && newEntry.value === "Fresh entry", "Recreated keyed text retained its old binding");
      oldEntry.dispatchEvent(new CompositionEvent("compositionend"));
      oldEntry.dispatchEvent(new Event("input"));
      await doc.flush();
      check(doc.current.cells.A1?.input === "Fresh entry", "A stale binding overwrote the recreated entry");
      newEntry.value = "Fresh entry edited";
      newEntry.dispatchEvent(new Event("input"));
      await doc.flush();
      check(doc.current.cells.A1?.input === "Fresh entry edited", "The recreated binding could not edit");
      // Slide-deck rows contain their own rows; moving a block preserves both identities.
      const slide = (await doc.fields.slides.insert({ title: "Edited slide" })).id;
      const blocks = doc.fields.slides.item(slide).blocks;
      const blockA = (await blocks.insert({ text: "Block A" })).id;
      const blockB = (await blocks.insert({ text: "Block B" })).id;
      await blocks.move(blockB, { before: blockA });
      await blocks.item(blockA).text.set("Block A edited");
      const renderedSlide = doc.current.slides.find((entry) => entry.$id === slide)!;
      check(renderedSlide.blocks.map((block) => block.$id).join() === [blockB, blockA].join() &&
        renderedSlide.blocks[1]?.text === "Block A edited", "Nested row identity or text was lost");
      // Counters add up; a change applies all of its writes or none.
      const hits = doc.current.hits;
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
      await doc.fields.hits.increment(); check(doc.current.hits === hits + 4, "Default increment failed");
      await doc.at(doc.current).epoch.set(Number.MAX_SAFE_INTEGER); check(doc.current.epoch === Number.MAX_SAFE_INTEGER, "Positive safe integer changed");
      await doc.at(doc.current).epoch.set(-Number.MAX_SAFE_INTEGER); check(doc.current.epoch === -Number.MAX_SAFE_INTEGER, "Negative safe integer changed");
      await doc.at(doc.current.settings).volume.set(6); check(doc.current.settings.volume === 6, "Nested doc.at failed");
      const txRow = doc.current.rows.find((r) => r.$id === first)!;
      await doc.change((tx) => { tx.at(txRow).text.set("Transaction row"); tx.fields.checkins.put("tx", 3); tx.fields.colors.replace(["tx", "remove", "remove"]); tx.fields.colors.remove(1, 2); });
      check(doc.current.rows.find((r) => r.$id === first)?.text === "Transaction row" && doc.current.checkins.tx === 3 && doc.current.colors.join() === "tx", "Transaction at/record/list/text failed");
      await refused(doc.fields.checkins.put("__proto__", 1), "invalid_key"); await refused(doc.fields.count.set(1.5), "type_mismatch");
      await doc.fields.cells.delete("missing"); await refused(doc.fields.cells.entry("missing").input.set("x"), "path_not_found");
      // Probe the ctx document directly: the Svelte adapter intentionally hides observers.
      const raw = mountedContext().document as TinyDocument<typeof schema.descriptor>; let callsA = 0, callsB = 0;
      const stopA = raw.subscribe(() => callsA++), stopB = raw.subscribe(() => callsB++); await doc.fields.label.set("Listeners");
      check(callsA > 0 && callsA === callsB, "Subscribers did not both receive publication"); stopA(); stopA(); const endedA = callsA; await doc.fields.label.set("One listener");
      check(callsA === endedA && callsB > callsA, "Double dispose removed another listener"); stopB(); stopB();
      let readsA = 0, readsB = 0; const endA = raw.observe(() => readsA++), endB = raw.observe(() => readsB++); endA(); endA();
      void raw.fields.done.value; check(readsA === 0 && readsB === 1, "Old observe disposer removed its replacement"); endB(); endB();
      // Undo and redo step through the person's edits.
      await doc.fields.label.set("Before undo");
      await doc.fields.label.set("Undo me");
      await doc.undo();
      check(doc.current.label === "Before undo", "Undo did not revert");
      await doc.redo();
      check(doc.current.label === "Undo me", "Redo did not reapply");
      check(Array.isArray(doc.current.rows), "Snapshot missing");
      // Host services: attachments and the window.
      const ref = await attachments.import<typeof schema.descriptor>(new File(["conformance"], "note.txt", { type: "text/plain" }), (tx, ref) =>
        tx.fields.attachment.set(ref.id),
      );
      check(doc.current.attachment === ref.id && ref.name === "note.txt" && ref.mimeType === "application/octet-stream", "Attachment reference was lost");
      check((await (await attachments.read(ref.id)).text()) === "conformance", "Attachment bytes differ");
      check((await (await fetch(attachments.url(ref.id))).text()) === "conformance", "Attachment URL bytes differ");
      check(document.querySelector("[data-tiny-root]") !== null, "The app root is not marked for the host");
      const html = document.documentElement, css = getComputedStyle(html);
      check(html.getAttribute("data-tiny-presentation") === "standard" && html.hasAttribute("data-tiny-resizable") && !html.hasAttribute("data-tiny-capture"), "Host presentation attributes changed");
      for (const token of ["accent", "ink", "paper-tint", "window-width", "window-height"]) check(css.getPropertyValue(`--tiny-${token}`).trim().length > 0, `Missing --tiny-${token}`);
      for (const [options, reason] of [[{}, "notDeclared"], [{ locale: "bad locale!" }, "locale"]] as const) {
        try { await speech.transcribe(ref.id, options); throw new Error("Speech unexpectedly succeeded"); } catch (error) { check(isSpeechError(error) && error.code === "unavailable" && error.reason === reason, `Expected speech ${reason}, got ${String(error)}`); }
      }
      document.querySelector<HTMLButtonElement>("[data-contract-refuse]")!.click(); await new Promise((resolve) => setTimeout(resolve, 0)); await tick();
      check(document.querySelector("[data-tiny-notice] p")?.textContent === "Conformance refusal", "Unhandled refuse did not render a notice");
      // The app's own media load from its embedded assets.
      for (const url of mediaURLs) check((await fetch(url)).ok, `App media did not load: ${url}`);
      await document.fonts.load('16px "Conformance Hand"');
      check(document.fonts.check('16px "Conformance Hand"'), "The imported font did not load");
      await resizeWindow({ width: 480, height: 480 }).catch((error) => {
        if (!isDocumentError(error)) throw error;
      });
      // A resolved flush means every accepted edit is saved.
      await doc.flush();
      return true;
    };
    return () => {
      delete (globalThis as any).contractTest;
    };
  });
</script>

<main>
  <h2>Conformance</h2>
  <button data-contract-refuse onclick={() => refuse("Conformance refusal")}>Refuse</button>
  <img src={swatch} alt="Swatch" width="16" height="16" />
  <audio src={tone} preload="none"></audio>
  <p bind:this={shown}>{doc.current.title}</p>
  <textarea aria-label="Title" bind:this={input} use:bindText={doc.fields.title}></textarea>
  <input aria-label="Done" type="checkbox" bind:this={checkbox} bind:checked={doc.fields.done.value} />
  {#if doc.current.cells.A1}
    <textarea aria-label="Keyed entry" use:bindText={doc.fields.cells.entry("A1").input}></textarea>
  {/if}
  <ul>
    {#each doc.current.rows as row (row.$id)}<li>{row.text}</li>{/each}
  </ul>
</main>
