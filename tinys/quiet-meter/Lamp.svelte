<script lang="ts">
  import type { Zone } from "./meter";
  // One big lamp: lit in a zone colour with the level inside, or dark when nothing is being measured.
  let { zone, value, caption }: { zone: Zone | "off"; value: number | null; caption: string } = $props();
</script>

<div class="qm-lamp" data-zone={zone}>
  <div class="qm-lamp-glass">
    <span class="qm-lamp-value">{value === null ? "--" : Math.round(value)}</span>
    <span class="qm-lamp-caption">{caption}</span>
  </div>
</div>
