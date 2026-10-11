<script lang="ts">
  // The tempo ladder: one rung per step from the start (bottom) to the goal (top).
  // Rungs at or below the current tempo are climbed.
  let { rungs, tempo }: { rungs: number[]; tempo: number } = $props();
  const top = $derived(rungs.length - 1);
  const reached = $derived(rungs.reduce((at, bpm, index) => (bpm <= tempo ? index : at), -1));
</script>

<div class="ps-ladder" role="img" aria-label={`Tempo ladder: ${tempo} bpm, rung ${reached + 1} of ${rungs.length}, goal ${rungs[top]} bpm`}>
  <span class="ps-ladder-end ps-ladder-goal">{rungs[top]}</span>
  <div class="ps-ladder-frame">
    {#each rungs as _, i (i)}
      {@const index = top - i}
      <i class="ps-rung" data-climbed={index <= reached} data-current={index === reached}></i>
    {/each}
  </div>
  <span class="ps-ladder-end">{rungs[0]}</span>
</div>
