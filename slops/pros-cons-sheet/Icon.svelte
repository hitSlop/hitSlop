<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { balanceView } from "./balance";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const { tilt } = $derived(balanceView(snapshot));
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconTile">
    <article class="iconSheet">
      <div class="iconHead">
        <div class="iconBeam" style:transform={`rotate(${tilt}deg)`}>
          <span class="iconBar"></span>
          <span class="iconPan" style:transform={`rotate(${-tilt}deg)`}></span>
          <span class="iconPan" style:transform={`rotate(${-tilt}deg)`}></span>
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
