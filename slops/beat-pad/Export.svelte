<script lang="ts">
  import doc from "./schema";
  import { beatView } from "./pattern";

  const beat = $derived(doc.current);
  const view = $derived(beatView(beat));
</script>

<article class="studio export" aria-label="Exported beat">
  <header class="top"><h1>Beat Pad</h1><p class="clock">{beat.bpm} <small>BPM</small> · {view.kit} · swing {beat.swing}%</p></header>
  <div class="grid">
    {#each view.bars as bar (bar.half)}
      <div class="bar-block">
        {#each bar.tracks as row (row.track)}
          <div class="track" data-track={row.track}>
            <span class="name">{row.name}</span>
            {#each row.cells as cell (cell.index)}
              <span class="cell static" data-on={cell.on} data-beat={cell.beat}>{cell.mark}</span>
            {/each}
          </div>
        {/each}
      </div>
    {/each}
  </div>
</article>
