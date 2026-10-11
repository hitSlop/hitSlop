<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, draft } from "hitslop/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { Checkbox, Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import Eraser from "@lucide/svelte/icons/rotate-ccw";
  import doc from "./schema";
  import { addItem, cardView, recoverAll, removeItem, revealItem, setDone } from "./items";
  import { scratch } from "./scratch";

  const newItem = draft((text) => doc.change((tx) => addItem(tx, text)));
  let composer = $state<HTMLInputElement>();
  const card = $derived(cardView(doc.current));

  async function addNewItem(event: SubmitEvent) {
    if (await newItem.submit(event)) composer?.focus();
  }

  function recoverCard() {
    doc.change((tx) => recoverAll(tx, doc.current));
  }
</script>

<main class="night" aria-label="Scratch-off bucket list">
  <article class="ticket">
    <header class="band">
      <p class="kicker">Instant win · every ticket a winner</p>
      <input class="title" aria-label="Card title" placeholder="Summer lucky 12" use:bindText={doc.fields.title} />
      <p class="tally"><b>{card.revealed}</b>/{card.total} scratched · <b>{card.done}</b> done</p>
    </header>
    <div class="perf" aria-hidden="true"></div>

    <ul class="grid">
      {#each card.items as item, index (item.$id)}
        {@const row = doc.at(item)}
        <li class="tile" data-revealed={item.revealed} data-done={item.done}>
          <span class="num" aria-hidden="true">{index + 1}</span>
          {#if item.revealed}
            <textarea class="what" rows="3" aria-label={`Item ${index + 1}`} placeholder="Something to try" use:bindText={row.text}></textarea>
          {:else}
            <span class="what covered" aria-hidden="true">{item.text}</span>
            <canvas class="foil" aria-hidden="true" use:scratch={{ onreveal: () => doc.change((tx) => revealItem(tx, item.$id)), reduced: prefersReducedMotion.current }}></canvas>
            <button class="reveal" aria-label={`Reveal item ${index + 1}`} onclick={() => doc.change((tx) => revealItem(tx, item.$id))}>Reveal</button>
          {/if}
          {#if item.revealed}
            <Checkbox.Root class="stamp-box" checked={item.done} onCheckedChange={(checked) => doc.change((tx) => setDone(tx, item.$id, checked === true))} aria-label={`${item.text || `Item ${index + 1}`}: mark ${item.done ? "not done" : "done"}`}>
              {#snippet children({ checked })}{#if checked}<Check size={12} strokeWidth={3.5} />Done{:else}Did it?{/if}{/snippet}
            </Checkbox.Root>
          {/if}
          <button class="x" aria-label={`Remove item ${index + 1}`} onclick={() => doc.change((tx) => removeItem(tx, item.$id))}><X size={12} /></button>
        </li>
      {:else}
        <li class="empty"><h2>The card is blank.</h2><p>Add something you want to try.</p></li>
      {/each}
    </ul>

    <form class="composer" onsubmit={addNewItem}>
      <input bind:this={composer} bind:value={newItem.value} aria-label="New item" placeholder="Add your own (it won’t be a surprise)" />
      <Button.Root class="add" type="submit" aria-label="Add item" disabled={!newItem.ready}><Plus size={18} /></Button.Root>
    </form>
    <footer class="foot">
      <Button.Root class="recover" onclick={recoverCard} disabled={!card.revealed}><Eraser size={14} />Re-cover all</Button.Root>
      <span>Rub the foil to reveal</span>
    </footer>
  </article>
</main>
