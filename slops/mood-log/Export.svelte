<script lang="ts">
  import doc from "./schema";
  import { moodView } from "./entries";
  import { orbColor } from "./orbs";

  const view = $derived(moodView(doc.current));
</script>

<article class="export-sheet" aria-label="Exported mood log">
  <h1 class="prompt">{doc.current.prompt}</h1>
  <ul class="list">
    {#each view.entries as item (item.$id)}
      <li class="row">
        <span class="entry-orb" style:background={orbColor(item.mood)}></span>
        <div class="meta"><span>{item.day}</span><small>⚡ {item.energy}/5</small></div>
        <p class="note">{item.note}</p>
      </li>
    {/each}
  </ul>
</article>
