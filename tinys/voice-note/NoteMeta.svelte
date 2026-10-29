<!-- One note's date line, the same in the editor and the export. -->
<script lang="ts">
  import { durationLabel, formatName, whenLabel, type Note } from "./notes";
  let { note }: { note: Note } = $props();
  const when = $derived(whenLabel(note.recordedAt));
</script>

<p class="vn-meta">
  <strong>{when.day}</strong>
  {#if when.time}<span>{when.time}</span>{/if}
  {#if note.clip}
    <span class="vn-meta-num">{durationLabel(note.durationMs)}</span>
    <span class="vn-chip">{formatName(note.clip)}</span>
  {:else}
    <span class="vn-chip vn-chip-typed">Typed</span>
  {/if}
</p>
