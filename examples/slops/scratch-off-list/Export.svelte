<script lang="ts">
  import doc from "./schema";

  const revealed = $derived(doc.current.items.filter((item) => item.revealed).length);
  const done = $derived(doc.current.items.filter((item) => item.done).length);
</script>

<article class="night export" aria-label="Exported scratch-off card">
  <div class="ticket">
    <header class="band">
      <p class="kicker">Instant win · every ticket a winner</p>
      <h1 class="title">{doc.current.title || "Summer lucky 12"}</h1>
      <p class="tally"><b>{revealed}</b>/{doc.current.items.length} scratched · <b>{done}</b> done</p>
    </header>
    <div class="perf" aria-hidden="true"></div>
    <ul class="grid">
      {#each doc.current.items as item, index (item.$id)}
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
