<script lang="ts">
  import doc from "./schema";
  import { percentOf } from "./intake";

  const percent = $derived(percentOf(doc.current.current, doc.current.target));
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
