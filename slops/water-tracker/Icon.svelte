<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { intakeView } from "./intake";

  let { snapshot }: CaptureProps<typeof schema> = $props();
  const percent = $derived(intakeView(snapshot).percent);
  const iconFill = $derived(Math.max(18, Math.min(86, percent)));
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconShell">
    <div class="iconChamber">
      <div class="iconFill" style:height={`${iconFill}%`}><div class="iconWave"></div></div>
      <span class="iconDrop">💧</span>
    </div>
    <div class="iconDots"><span></span><span></span><span></span><span></span></div>
  </div>
</div>
