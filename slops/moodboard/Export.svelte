<script lang="ts">
  import { onMount, tick } from "svelte";
  import { attachments, capture } from "hitslop/svelte";
  import doc from "./schema";
  import { boardView } from "./tiles";

  // A contact sheet of every image, not the live tunnel: the export never renders WebGL.
  const view = $derived(boardView(doc.current));

  onMount(() =>
    capture.onPrepare(async () => {
      await tick();
      await Promise.all(Array.from(document.images, (image) => image.decode().catch(() => undefined)));
    }),
  );
</script>

<article class="wall export" aria-label="Exported moodboard">
  <h1>{view.title}</h1>
  <ul class="sheet">
    {#each view.tiles as tile (tile.$id)}
      <li>
        <img src={attachments.url(tile.image.id)} alt={tile.caption || "Board image"} decoding="sync" />
        {#if tile.caption}<p>{tile.caption}</p>{/if}
      </li>
    {:else}
      <li class="none"><p>An empty board.</p></li>
    {/each}
  </ul>
</article>
