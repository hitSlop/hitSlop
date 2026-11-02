<script lang="ts">
  let { tilt, proTotal, conTotal }: { tilt: number; proTotal: number; conTotal: number } = $props();
  const net = $derived(proTotal - conTotal);
  const lean = $derived(net > 0 ? "for" : net < 0 ? "against" : "even");
</script>

<div class="scale" aria-live="polite">
  <div class="beamHold" aria-hidden="true">
    <div class="post"></div>
    <div class="fulcrum"></div>
    <div class="beam" style:transform={`rotate(${tilt}deg)`}>
      <div class="bar"></div>
      <div class="hang" style:transform={`rotate(${-tilt}deg)`}>
        <span class="chain"></span>
        <div class="pan" data-side="for">{proTotal}</div>
      </div>
      <div class="hang" style:transform={`rotate(${-tilt}deg)`}>
        <span class="chain"></span>
        <div class="pan" data-side="against">{conTotal}</div>
      </div>
    </div>
  </div>
  <p class="delta" data-lean={lean}>
    {#if net > 0}
      For by {net}
    {:else if net < 0}
      Against by {Math.abs(net)}
    {:else}
      Even balance
    {/if}
  </p>
</div>
