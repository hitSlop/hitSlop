<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { moodView } from "./entries";
  import { orbColor } from "./orbs";

  const view = $derived(moodView(doc.current));
</script>

<article class="export-sheet" aria-label="Exported mood log">
  <h1 class="prompt">{doc.current.prompt}</h1>
  <ul class="list">
    {#each (preview ? (view.entries).slice(0, 6) : view.entries) as item (item.$id)}
      <li class="row">
        <span class="entry-orb" style:background={orbColor(item.mood)}></span>
        <div class="meta"><span>{item.day}</span><small>⚡ {item.energy}/5</small></div>
        <p class="note">{previewText(item.note, preview, 140)}</p>
      </li>
    {/each}
  </ul>
</article>
