<script lang="ts">
    let { words = 0 }: { words?: number } = $props();
  const complete = $derived(words >= 750);
  const label = $derived(complete ? "750" : String(Math.max(0, Math.round(words))).padStart(3, "0"));
</script>

<div class={"mp-iconSurface"} aria-hidden="true">
  <div class={"mp-iconPad"}>
    <span class={"mp-iconMargin"}></span>
    <div class={"mp-iconLines"}>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
      <span class={"mp-iconLine"}></span>
    </div>
    <span class={"mp-iconSeal"} data-complete={complete}>{label}</span>
  </div>
</div>
