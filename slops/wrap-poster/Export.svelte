<script lang="ts">
  import doc from "./schema";
  import Poster from "./Poster.svelte";
  import { posterView } from "./poster";

  const view = $derived(posterView(doc.current));
</script>

<article class="export" aria-label="Exported poster">
  <Poster title={doc.current.title} body={doc.current.body} shapes={view.shapes} />
</article>
