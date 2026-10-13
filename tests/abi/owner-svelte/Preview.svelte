<script lang="ts">
import type { CaptureProps } from "hitslop/svelte";
import type schema from "./schema";
let { snapshot }: CaptureProps<typeof schema> = $props();
const mode = "preview";
</script>

<main data-capture-mode={mode}>
  <h1>{snapshot.title}</h1>
  <p>{snapshot.rows.length} rows</p>
  {#each snapshot.slides as slide (slide.$id)}
    <section aria-label={slide.title}>
      <h2>{slide.title}</h2>
      {#each slide.blocks as block (block.$id)}<p>{block.text}</p>{/each}
    </section>
  {/each}
</main>
