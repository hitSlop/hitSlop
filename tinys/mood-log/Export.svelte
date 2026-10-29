<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { previewText } from "../_shared/preview";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { moodView } from "./entries";
  import { orbColor } from "./orbs";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(moodView(snapshot));
</script>

<article class="export-sheet" aria-label="Exported mood log">
  <h1 class="prompt">{snapshot.prompt}</h1>
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
