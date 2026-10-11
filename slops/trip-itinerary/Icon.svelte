<script lang="ts">
  import doc from "./schema";
  import { itineraryView } from "./shared";

  const view = $derived(itineraryView(doc.current));
</script>

<div class="icon-surface" aria-hidden="true">
  <article class="icon-pass">
    <div class="icon-main">
      <div class="icon-head">
        <span class="icon-airline">HITSLOP AIRWAYS</span>
        <span class="icon-class">BOARDING</span>
      </div>
      <div class="icon-route">
        <div class="icon-city"><strong>{doc.current.origin || "SFO"}</strong><span>{doc.current.originCity || "ORIGIN"}</span></div>
        <span class="icon-plane">✈</span>
        <div class="icon-city"><strong>{doc.current.destination || "NRT"}</strong><span>{doc.current.destCity || "DEST"}</span></div>
      </div>
      <div class="icon-meta">
        <div><span>FLIGHT</span><strong>{doc.current.flight || "—"}</strong></div>
        <div><span>GATE</span><strong>{doc.current.gate || "—"}</strong></div>
        <div><span>SEAT</span><strong>{doc.current.seat || "—"}</strong></div>
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
