<script lang="ts">
  import doc from "./schema";
  import { ui } from "./ui.svelte";

  const count = $derived(doc.current.props.length);
</script>

<figure class="planet-export">
  {#if ui.snapshot}<img src={ui.snapshot} alt={`${doc.current.name || "A tiny planet"}, a low-poly world in ${doc.current.season}`} />{/if}
  <figcaption><strong>{doc.current.name || "Tiny Planet"}</strong><span>{count} {count === 1 ? "thing" : "things"} · {doc.current.season}</span></figcaption>
</figure>
