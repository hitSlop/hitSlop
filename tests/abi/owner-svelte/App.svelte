<script lang="ts">
  import { tick, onMount } from "svelte";
  import { bindText } from "@hitslop/document/svelte";
  import doc from "./schema";
  let shown: HTMLParagraphElement;
  let input: HTMLTextAreaElement;
  let checkbox: HTMLInputElement;
  onMount(() => {
    (globalThis as any).contractTest = async () => {
      await doc.fields.title.set("Accepted 😀");
      if (doc.current.title !== "Accepted 😀") throw new Error("Acceptance did not publish");
      await tick();
      if (shown.textContent !== "Accepted 😀") throw new Error("Svelte did not render publication");
      input.value = "Draft 中文";
      input.dispatchEvent(new CompositionEvent("compositionstart"));
      input.dispatchEvent(new Event("input"));
      if (doc.current.title !== "Accepted 😀") throw new Error("Composition sent early");
      input.dispatchEvent(new CompositionEvent("compositionend"));
      await doc.flush();
      if (doc.current.title !== "Draft 中文") throw new Error("Draft was lost");
      checkbox.checked = true;
      checkbox.dispatchEvent(new Event("change"));
      if (!doc.current.done) throw new Error("Bound value did not show at once");
      await doc.flush();
      if (!doc.current.done) throw new Error("Boolean binding lost edit");
      await doc.fields.done.set(false);
      await tick();
      if (checkbox.checked) throw new Error("Bound value did not follow the document");
      const id = await doc.change(tx => {
        const { id } = tx.fields.rows.insert({ text: "Added", done: false });
        tx.fields.rows.item(id).done.set(true);
        return id;
      });
      if (!doc.current.rows.find(row => row.$id === id)?.done) throw new Error("Collector failed");
      // A resolved flush means every accepted edit is saved.
      await doc.flush();
      return true;
    };
    return () => { delete (globalThis as any).contractTest; };
  });
</script>

  <main><p bind:this={shown}>{doc.current.title}</p>
    <textarea aria-label="Title" bind:this={input} use:bindText={doc.fields.title}></textarea>
    <input aria-label="Done" type="checkbox" bind:this={checkbox} bind:checked={doc.fields.done.value} />
  </main>
