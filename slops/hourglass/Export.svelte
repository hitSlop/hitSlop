<script lang="ts">
import type schema from "./schema";
import Vessel from "./Vessel.svelte";
import { read } from "./model";
import type { CaptureMode, CaptureProps } from "hitslop/svelte";
let { snapshot, mode }: CaptureProps<typeof schema> & { mode: Exclude<CaptureMode, "icon"> } = $props();
const reading = $derived(read(snapshot, Date.now()));
</script>

<!-- A capture can't hold the window's frost, so the export paints its own glass and a
     solid page behind it. -->
<article class="hourglass-export" data-mode={mode}>
  <div class="hourglass-export-art"><Vessel remaining={reading.remaining} solid /></div>
  <div class="hourglass-export-text">
    <h1>{snapshot.title || "Hourglass"}</h1>
    {#if reading.state === "running"}
      <p class="hourglass-export-value"><strong>{reading.value}</strong> {reading.unit}</p>
      <p>until {reading.until}</p>
    {:else if reading.state === "done"}
      <p class="hourglass-export-value"><strong>Time's up</strong></p>
      <p>since {reading.until}</p>
    {:else}
      <p>Ready to turn over.</p>
    {/if}
  </div>
</article>
