<script lang="ts">
  import doc from "./schema";
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconPlate">
    <span class="iconRibbon"></span>
    {#each ["•", "×", "›", "○"] as mark}
      <div class="iconRow">
        <span class="iconBullet">{mark}</span>
        <i class="iconLine"></i>
      </div>
    {/each}
  </div>
</div>
