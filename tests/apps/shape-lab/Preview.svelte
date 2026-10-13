<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import variant from "./variant";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const mode = "preview";
</script>

<article class="shape-lab-export">
  <span class="shape-lab-corner shape-lab-tl">TL</span><span class="shape-lab-corner shape-lab-tr">TR</span>
  <span class="shape-lab-corner shape-lab-bl">BL</span><span class="shape-lab-corner shape-lab-br">BR</span>
  <p class="shape-lab-caption">SHAPE LAB / {mode.toUpperCase()}</p>
  <h1>Four corners. One rectangle.</h1><p>{variant.name}</p>
  <p class="shape-lab-export-note">{snapshot.note}</p>
  <p>{snapshot.hits} accepted clicks · Last: {snapshot.lastTarget}</p>
  <p>Window geometry must not cut this sheet.</p>
</article>
