<script lang="ts">
  import doc from "./schema";
  import { STEPS, kitLabel, trackLabel, tracks } from "./pattern";

  const beat = $derived(doc.current);
</script>

<article class="studio export" aria-label="Exported beat">
  <header class="top"><h1>Beat Pad</h1><p class="clock">{beat.bpm} <small>BPM</small> · {kitLabel[beat.kit]} · swing {beat.swing}%</p></header>
  <div class="grid">
    {#each [0, 1] as half}
      <div class="bar-block">
        {#each tracks as track}
          <div class="track" data-track={track}>
            <span class="name">{trackLabel[track]}</span>
            {#each Array.from({ length: STEPS / 2 }, (_, i) => half * (STEPS / 2) + i) as index (index)}
              {@const value = beat[track][index] ?? "."}
              <span class="cell static" data-on={value !== "."} data-beat={index % 4 === 0}>{#if value !== "."}{track === "bass" ? value : "●"}{/if}</span>
            {/each}
          </div>
        {/each}
      </div>
    {/each}
  </div>
</article>
