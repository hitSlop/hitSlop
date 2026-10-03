<script lang="ts">
  import doc from "./schema";
  import Poster from "./Poster.svelte";
  import type { PosterShape } from "./types";

  const shapes = $derived<PosterShape[]>(doc.current.shapes.map((shape) => ({ id: shape.$id, kind: shape.kind, tone: shape.tone, x: shape.x, y: shape.y, size: shape.size })));
</script>

<article class="export" aria-label="Exported poster">
  <Poster title={doc.current.title} body={doc.current.body} {shapes} />
</article>
