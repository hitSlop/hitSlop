<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  import { attachments } from "hitslop/svelte";
  import doc from "./schema";
  import { boardView } from "./tiles";

  // A contact sheet of every image, not the live tunnel: the export never renders WebGL.
  const view = $derived(boardView(doc.current));

</script>

<article class="wall export" aria-label="Exported moodboard">
  <h1>{view.title}</h1>
  <ul class="sheet">
    {#each (preview ? (view.tiles).slice(0, 6) : view.tiles) as tile (tile.$id)}
      <li>
        <img src={attachments.url(tile.image.id)} alt={tile.caption || "Board image"} decoding="sync" />
        {#if tile.caption}<p>{previewText(tile.caption, preview, 70)}</p>{/if}
      </li>
    {:else}
      <li class="none"><p>An empty board.</p></li>
    {/each}
  </ul>
</article>
