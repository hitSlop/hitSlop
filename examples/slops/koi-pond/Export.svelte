<script lang="ts">
  import doc from "./schema";
  import { list } from "./shared";
  import { ui } from "./ui.svelte";

  const feedings = $derived(doc.current.feedings);

  const feedingLabel = $derived(feedings === null ? "Feeding count unavailable" : `Fed ${feedings} ${feedings === 1 ? "time" : "times"}`);
</script>

<figure class="koi-export">
  {#if ui.snapshot}<img src={ui.snapshot} alt={`Watercolor koi pond with ${list(doc.current.koi.map(k => k.name.trim() || "unnamed"))}`} />{/if}
  <figcaption><strong>Koi Pond</strong><span>{doc.current.koi.length ? list(doc.current.koi.map(k => k.name.trim() || "unnamed")) : "A quiet pond"} · {feedingLabel}</span></figcaption>
</figure>
