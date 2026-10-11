<script lang="ts">
  import { zoneOf } from "./meter";
  // 25 lamps from 0 to 100. Their colours follow the thresholds; `value` lights them up to that level.
  let { value, amber, red }: { value: number | null; amber: number; red: number } = $props();
  const cells = Array.from({ length: 25 }, (_, i) => i * 4);
</script>

<div class="qm-gauge" aria-hidden="true">
  {#each cells as start}
    <i data-zone={zoneOf(start + 2, amber, red)} data-lit={value !== null && value > start + 1}></i>
  {/each}
</div>
