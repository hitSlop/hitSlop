<script lang="ts">
  import { LayerCake } from "layercake";
  import Layers from "./Layers.svelte";
  import type { Plan } from "./schema";

  interface Props { plan: Pick<Plan, "sources" | "buckets" | "currency">; interactive?: boolean; width?: number; height?: number }
  let { plan, interactive = false, width = 520, height = 300 }: Props = $props();
</script>

<div class="chart" style:height="{height}px">
  <LayerCake containerWidth={width} containerHeight={height} padding={{ top: 14, right: 118, bottom: 14, left: 118 }}>
    <Layers {plan} {interactive} />
  </LayerCake>
</div>
