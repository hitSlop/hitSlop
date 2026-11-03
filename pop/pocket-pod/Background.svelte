<script lang="ts">
import type { Data } from "./pod";
let { source, fit, dim }: { source?: string; fit: Data["backgroundFit"]; dim: number } = $props();
</script>

{#if source}
  <div class="pod-background" aria-hidden="true">
    <img src={source} alt="" style:object-fit={fit} />
    <div class="pod-background-shade" style:opacity={dim}></div>
  </div>
{/if}
