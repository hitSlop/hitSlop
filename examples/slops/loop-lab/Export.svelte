<script lang="ts">
  import doc from "./schema";
</script>

<article class="lab export" aria-label="Exported loops">
  <header class="top"><div class="titles"><h1>Loop Lab</h1></div><p class="clock">{doc.current.bpm} <small>BPM</small></p></header>
  <ul class="tracks">
    {#each doc.current.tracks as track (track.$id)}
      <li class="track" data-kind={track.kind} data-muted={track.muted}>
        <div class="line"><span class="name">{track.name}</span><span class="tag">{track.kind === "synth" ? track.voice : "samples"}{track.muted ? " · muted" : ""}</span></div>
        <p class="code static">{track.code}</p>
      </li>
    {:else}
      <li class="empty">No tracks yet.</li>
    {/each}
  </ul>
</article>
