<script lang="ts">
  import { drawPetIcon } from "./icon-draw";
  import { ui } from "./ui.svelte";

  let canvas = $state<HTMLCanvasElement>();
  $effect(() => { if (canvas && ui.image) drawPetIcon(canvas, ui.image); });
</script>

<div class="pet-icon" aria-label="Exported Codex Pet">
  <canvas bind:this={canvas} class="pet-icon-canvas" width="512" height="512"></canvas>
</div>
