<script lang="ts">
  import doc from "./schema";
  import { beamTilt, totalWeight } from "./balance";

  const pros = $derived(doc.current.factors.filter((item) => item.side === "pro"));

  const cons = $derived(doc.current.factors.filter((item) => item.side === "con"));

  const proTotal = $derived(totalWeight(pros));

  const conTotal = $derived(totalWeight(cons));

  const tiltTarget = $derived(beamTilt(proTotal, conTotal));
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconTile">
    <article class="iconSheet">
      <div class="iconHead">
        <div class="iconBeam" style:transform={`rotate(${tiltTarget}deg)`}>
          <span class="iconBar"></span>
          <span class="iconPan" style:transform={`rotate(${-tiltTarget}deg)`}></span>
          <span class="iconPan" style:transform={`rotate(${-tiltTarget}deg)`}></span>
        </div>
      </div>
      <div class="iconCols">
        <div class="iconCol">
          <span class="iconLine" data-side="for"></span>
          <span class="iconLine" data-side="for"></span>
          <span class="iconLine" data-side="for"></span>
        </div>
        <div class="iconCol">
          <span class="iconLine" data-side="against"></span>
          <span class="iconLine" data-side="against"></span>
        </div>
      </div>
    </article>
  </div>
</div>
