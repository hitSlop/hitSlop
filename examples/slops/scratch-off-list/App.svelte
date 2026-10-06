<script lang="ts">
  import { bindText } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { Checkbox, Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import Eraser from "@lucide/svelte/icons/rotate-ccw";
  import doc from "./schema";
  import { scratch } from "./scratch";

  let draft = $state("");
  const revealed = $derived(doc.current.items.filter((item) => item.revealed).length);
  const done = $derived(doc.current.items.filter((item) => item.done).length);

  function addItem() {
    const text = draft.trim();
    if (!text) return;
    doc.fields.items.insert({ text, revealed: true, done: false });
    draft = "";
  }

  function recoverAll() {
    doc.change((tx) => {
      for (const item of doc.current.items) {
        const row = tx.at(item);
        row.revealed.set(false);
        row.done.set(false);
      }
    });
  }
</script>

<main class="night" data-slop-selection="none" aria-label="Scratch-off bucket list">
  <article class="ticket">
    <header class="band">
      <p class="kicker">Instant win · every ticket a winner</p>
      <input class="title" aria-label="Card title" placeholder="Summer lucky 12" use:bindText={doc.fields.title} />
      <p class="tally"><b>{revealed}</b>/{doc.current.items.length} scratched · <b>{done}</b> done</p>
    </header>
    <div class="perf" aria-hidden="true"></div>

    <ul class="grid">
      {#each doc.current.items as item, index (item.$id)}
        {@const row = doc.at(item)}
        <li class="tile" data-revealed={item.revealed} data-done={item.done}>
          <span class="num" aria-hidden="true">{index + 1}</span>
          {#if item.revealed}
            <textarea class="what" rows="3" aria-label={`Item ${index + 1}`} placeholder="Something to try" use:bindText={row.text}></textarea>
          {:else}
            <span class="what covered" aria-hidden="true">{item.text}</span>
            <canvas class="foil" aria-hidden="true" use:scratch={{ onreveal: () => row.revealed.set(true), reduced: prefersReducedMotion.current }}></canvas>
            <button class="reveal" aria-label={`Reveal item ${index + 1}`} onclick={() => row.revealed.set(true)}>Reveal</button>
          {/if}
          {#if item.revealed}
            <Checkbox.Root class="stamp-box" checked={item.done} onCheckedChange={(checked) => row.done.set(checked === true)} aria-label={`${item.text || `Item ${index + 1}`}: mark ${item.done ? "not done" : "done"}`}>
              {#snippet children({ checked })}{#if checked}<Check size={12} strokeWidth={3.5} />Done{:else}Did it?{/if}{/snippet}
            </Checkbox.Root>
          {/if}
          <button class="x" aria-label={`Remove item ${index + 1}`} onclick={() => doc.fields.items.remove(item.$id)}><X size={12} /></button>
        </li>
      {:else}
        <li class="empty"><h2>The card is blank.</h2><p>Add something you want to try.</p></li>
      {/each}
    </ul>

    <form class="composer" onsubmit={(event) => { event.preventDefault(); addItem(); }}>
      <input bind:value={draft} aria-label="New item" placeholder="Add your own (it won’t be a surprise)" />
      <Button.Root class="add" type="submit" aria-label="Add item" disabled={!draft.trim()}><Plus size={18} /></Button.Root>
    </form>
    <footer class="foot">
      <Button.Root class="recover" onclick={recoverAll} disabled={!revealed}><Eraser size={14} />Re-cover all</Button.Root>
      <span>Rub the foil to reveal</span>
    </footer>
  </article>
</main>
