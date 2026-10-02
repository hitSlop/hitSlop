<script lang="ts">
  import { orbs } from "./orbs";
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-sheet">
    <div class="icon-title"></div>
    <div class="icon-orbs">
      {#each orbs as orb, index}<span style:background={orb.color} data-active={index === 4}></span>{/each}
    </div>
    <div class="icon-line"></div>
    <div class="icon-line"></div>
    <div class="icon-line"></div>
  </div>
</div>
