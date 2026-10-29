<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { cardView } from "./items";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const card = $derived(cardView(snapshot));
</script>

<article class="night export" aria-label="Exported scratch-off card">
  <div class="ticket">
    <header class="band">
      <p class="kicker">Instant win · every ticket a winner</p>
      <h1 class="title">{card.title}</h1>
      <p class="tally"><b>{card.revealed}</b>/{card.total} scratched · <b>{card.done}</b> done</p>
    </header>
    <div class="perf" aria-hidden="true"></div>
    <ul class="grid">
      {#each (preview ? (card.items).slice(0, 8) : card.items) as item, index (item.$id)}
        <li class="tile" data-revealed={item.revealed} data-done={item.done}>
          <span class="num">{index + 1}</span>
          {#if item.revealed}
            <span class="what">{item.text || "Something to try"}</span>
            {#if item.done}<span class="stamp-box" data-state="checked">✓ Done</span>{/if}
          {:else}
            <span class="foil static" aria-label="Still covered">?</span>
          {/if}
        </li>
      {:else}
        <li class="empty"><h2>The card is blank.</h2></li>
      {/each}
    </ul>
  </div>
</article>
