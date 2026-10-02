<script lang="ts">
  import doc from "./schema";
  import { orbColor } from "./orbs";
</script>

<article class="export-sheet" aria-label="Exported mood log">
  <h1 class="prompt">{doc.current.prompt}</h1>
  <ul class="list">
    {#each doc.current.entries as item (item.$id)}
      <li class="row">
        <span class="entry-orb" style:background={orbColor(item.mood)}></span>
        <div class="meta"><span>{item.day}</span><small>⚡ {item.energy}/5</small></div>
        <p class="note">{item.note}</p>
      </li>
    {/each}
  </ul>
</article>
