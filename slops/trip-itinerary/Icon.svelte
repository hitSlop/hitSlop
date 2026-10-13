<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { itineraryView } from "./shared";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const view = $derived(itineraryView(snapshot));
</script>

<div class="icon-surface" aria-hidden="true">
  <article class="icon-pass">
    <div class="icon-main">
      <div class="icon-head">
        <span class="icon-airline">HITSLOP AIRWAYS</span>
        <span class="icon-class">BOARDING</span>
      </div>
      <div class="icon-route">
        <div class="icon-city"><strong>{snapshot.origin || "SFO"}</strong><span>{snapshot.originCity || "ORIGIN"}</span></div>
        <span class="icon-plane">✈</span>
        <div class="icon-city"><strong>{snapshot.destination || "NRT"}</strong><span>{snapshot.destCity || "DEST"}</span></div>
      </div>
      <div class="icon-meta">
        <div><span>FLIGHT</span><strong>{snapshot.flight || "—"}</strong></div>
        <div><span>GATE</span><strong>{snapshot.gate || "—"}</strong></div>
        <div><span>SEAT</span><strong>{snapshot.seat || "—"}</strong></div>
      </div>
    </div>
    <div class="icon-perf">
      <span class="icon-notch icon-notch-top"></span>
      <span class="icon-notch icon-notch-bottom"></span>
    </div>
    <div class="icon-stub">
      <div class="icon-barcode"></div>
      <span class="icon-code">{view.boarded ? "BOARDED" : "OPEN"}</span>
    </div>
  </article>
</div>
