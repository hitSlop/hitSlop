<script lang="ts">
  import { tick, onMount } from "svelte";
  import { Slop, useDocument, bindText, bindValue } from "@hitslop/document/svelte";
  import schema from "./schema";
  const doc = useDocument(schema);
  let shown: HTMLParagraphElement;
  let input: HTMLTextAreaElement;
  let checkbox: HTMLInputElement;
  onMount(() => {
    (globalThis as any).contractTest = async () => {
      await doc.fields.title.replace("Accepted 😀");
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
      await doc.flush();
      if (!doc.current.done) throw new Error("Boolean binding lost edit");
      const id = await doc.change(tx => {
        const { id } = tx.fields.rows.insert({ text: "Added", done: false });
        tx.fields.rows.item(id).done.set(true);
        return id;
      });
      if (!doc.current.rows.find(row => row.$id === id)?.done) throw new Error("Collector failed");
      await doc.flush();
      return doc.status === "saved";
    };
    return () => { delete (globalThis as any).contractTest; };
  });
</script>
<Slop>
  <main><p bind:this={shown}>{doc.current.title}</p>
    <textarea aria-label="Title" bind:this={input} use:bindText={doc.fields.title}></textarea>
    <input aria-label="Done" type="checkbox" bind:this={checkbox} use:bindValue={doc.fields.done} />
  </main>
  {#snippet exportView()}<main><h1>{doc.current.title}</h1><p>{doc.current.rows.length} rows</p></main>{/snippet}
  {#snippet icon()}<main>ABI 2</main>{/snippet}
</Slop>
