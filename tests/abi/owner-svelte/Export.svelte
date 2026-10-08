<script lang="ts">
import doc from "./schema";
let { mode }: { mode: "preview" | "export" } = $props();
</script>

<main data-capture-mode={mode}>
  <h1>{doc.current.title}</h1>
  <p>{doc.current.rows.length} rows</p>
  {#each doc.current.slides as slide (slide.$id)}
    <section aria-label={slide.title}>
      <h2>{slide.title}</h2>
      {#each slide.blocks as block (block.$id)}<p>{block.text}</p>{/each}
    </section>
  {/each}
</main>
