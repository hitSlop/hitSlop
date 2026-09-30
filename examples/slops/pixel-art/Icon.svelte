<script lang="ts">

const FACE = new Set([8, 9, 14, 15, 16, 17, 22, 23, 25, 26, 29, 30, 33, 34, 37, 38, 41, 42, 43, 44, 45, 46, 49, 54]);
</script>

<div class="iconSurface" aria-hidden="true">
    <article class="iconBody">
      <div class="iconBezel">
        <div class="iconScreen">
          {#each Array.from({ length: 64 }, (_, index) => index) as index}
            <i class="iconPixel" data-on={FACE.has(index)}></i>
          {/each}
        </div>
      </div>
      <div class="iconControls">
        <span class="iconDpad"></span>
        <span class="iconBtns"><i class="iconBtn iconBtnOffset"></i><i class="iconBtn"></i></span>
      </div>
    </article>
  </div>
