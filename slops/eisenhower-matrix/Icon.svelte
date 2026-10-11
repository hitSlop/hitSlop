<script lang="ts">
  import doc from "./schema";
  import { matrixView } from "./shared";

  const view = $derived(matrixView(doc.current));
</script>

<div class="iconSurface" aria-hidden="true">
  <article class="iconSheet">
    <div class="iconHead">
      <span class="iconTitle"></span>
      <span class="iconMeter"><span class="iconMeterFill" style:width="{Math.max(view.q2Ratio, 12)}%"></span></span>
    </div>
    <div class="iconGrid">
      <span class="iconQuad" data-quad="q1" data-filled={view.filled.q1}></span>
      <span class="iconQuad" data-quad="q2" data-filled={view.filled.q2}></span>
      <span class="iconQuad" data-quad="q3" data-filled={view.filled.q3}></span>
      <span class="iconQuad" data-quad="q4" data-filled={view.filled.q4}></span>
    </div>
  </article>
</div>
