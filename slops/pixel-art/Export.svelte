<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { spriteView } from "./sprite";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const { cells } = $derived(spriteView(snapshot));
</script>

<article class="exportSheet" aria-label="Exported 16 by 16 sprite">
    <div class="exportBezel">
      <svg class="exportPixels" width="384" height="384" viewBox="0 0 16 16" shape-rendering="crispEdges" aria-label="Saved pixel art">
        {#each cells as color, index (index)}
          <rect x={index % 16} y={Math.floor(index / 16)} width="1" height="1" fill={color || ((index % 16 + Math.floor(index / 16)) % 2 ? "var(--slop-screenLite)" : "var(--slop-screenMid)")} />
        {/each}
      </svg>
    </div>
  </article>
