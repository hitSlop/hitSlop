<script lang="ts">
  import { keyLabel, type TuneHeader } from "./practice";
  let { header }: { header: TuneHeader } = $props();
  const facts = $derived([header.key && keyLabel(header.key), header.meter && `${header.meter} time`].filter(Boolean));
</script>

<div class="ps-titleblock">
  <h1 class="ps-title">{header.title || "Untitled piece"}</h1>
  <p class="ps-byline">
    {#if header.composer}<span class="ps-composer">{header.composer}</span>{/if}
    {#each facts as fact}<span class="ps-fact">{fact}</span>{/each}
  </p>
</div>
