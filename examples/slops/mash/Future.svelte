<script lang="ts">
  import doc, { slots } from "./schema";
  import { play, slotLabel } from "./fate";

  const fate = $derived(play(doc.current.options, doc.current.loops));
  const picked = $derived(slots.flatMap((slot) => {
    const option = doc.current.options.find((o) => o.$id === fate.picks[slot]);
    return option ? [{ slot, text: option.text }] : [];
  }));
</script>

<aside class="sticky" aria-label="Your future">
  <span class="tape" aria-hidden="true"></span>
  <h2>{doc.current.title.trim() ? `${doc.current.title.trim()}’s future` : "Your future"}</h2>
  {#if doc.current.loops === 0}
    <p class="waiting">Spin the spiral to find out…</p>
  {:else}
    <dl>
      {#each picked as row (row.slot)}
        <div><dt>{slotLabel[row.slot]}</dt><dd>{row.text}</dd></div>
      {/each}
    </dl>
  {/if}
</aside>
