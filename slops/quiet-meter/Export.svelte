<script lang="ts">
  import doc from "./schema";
  import { meterView, ZONE_LABEL } from "./meter";
  import Gauge from "./Gauge.svelte";
  import Lamp from "./Lamp.svelte";
  import Summary from "./Summary.svelte";
  // The export never listens: it shows the thresholds and the last saved summary, not a live level.
  const { last, lastZone, recent, room } = $derived(meterView(doc.current));
</script>

<main class="qm-shell qm-export">
  <p class="qm-eyebrow">Quiet Meter</p>
  <h1 class="qm-room">{room}</h1>
  <section class="qm-housing" aria-label="Last saved peak">
    <Lamp zone={lastZone ?? "off"} value={last ? last.peak : null} caption={last ? "peak, last session" : "no session yet"} />
    <p class="qm-status" data-zone={lastZone ?? "off"}>{lastZone ? ZONE_LABEL[lastZone] : "Not listening"}</p>
    <Gauge value={last ? last.peak : null} amber={doc.current.amber} red={doc.current.red} />
  </section>
  <section class="qm-panel" aria-label="Thresholds">
    <h2>Thresholds</h2>
    <ul class="qm-limits">
      <li data-zone="warn"><b>Amber</b> from {doc.current.amber}</li>
      <li data-zone="loud"><b>Red</b> from {doc.current.red}</li>
      <li><b>Too loud</b> after {doc.current.sustain} s of red</li>
    </ul>
    <h2>Last session</h2>
    <Summary session={last} zone={lastZone} />
    {#if recent.length > 1}
      <ol class="qm-history">
        {#each recent.slice(1) as row (row.$id)}
          <li><span>{row.date}</span><span>{row.episodes} {row.episodes === 1 ? "episode" : "episodes"}</span><span>peak {row.peak}</span></li>
        {/each}
      </ol>
    {/if}
  </section>
</main>
