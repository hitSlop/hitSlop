<script lang="ts">
  import { capture } from "hitslop/svelte";
  import { onMount } from "svelte";
  import { drawPetIcon } from "./icon-draw";
  import { petView } from "./pet";
  import { loadPet } from "./sheet";
  import doc from "./schema";

  let { label }: { label?: string } = $props();
  let canvas: HTMLCanvasElement;

  // Captures are fresh pages: load the saved pet's sheet here and hold the capture until it is painted.
  onMount(() => {
    let release = () => {};
    let disposed = false;
    const painted = loadPet(petView(doc.current).stored).then((loaded) => {
      if (disposed) { loaded.release(); return; }
      release = loaded.release;
      drawPetIcon(canvas, loaded.image);
    });
    const stop = capture.onPrepare(() => painted);
    return () => { disposed = true; stop(); release(); };
  });
</script>

<div class="pet-icon" aria-label={label} aria-hidden={label ? undefined : "true"}>
  <canvas bind:this={canvas} class="pet-icon-canvas" width="512" height="512"></canvas>
</div>
