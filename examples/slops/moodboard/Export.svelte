<script lang="ts">
  import doc from "./schema";
  import { ui } from "./ui.svelte";
</script>

<article class="wall export" aria-label="Exported moodboard">
  <h1>{doc.current.title || "Moodboard"}</h1>
  <ul class="sheet">
    {#each doc.current.tiles as tile (tile.$id)}
      <li>
        {#if ui.urls[tile.image.id]}<img src={ui.urls[tile.image.id]} alt={tile.caption || "Board image"} decoding="sync" />{:else}<span class="ph"></span>{/if}
        {#if tile.caption}<p>{tile.caption}</p>{/if}
      </li>
    {:else}
      <li class="none"><p>An empty board.</p></li>
    {/each}
  </ul>
</article>
