<script lang="ts">
    import doc from "./schema";
  import variant from "./variant";
  let { mode }: { mode: "preview" | "export" } = $props();
</script>

<article class="shape-lab-export">
  <span class="shape-lab-corner shape-lab-tl">TL</span><span class="shape-lab-corner shape-lab-tr">TR</span>
  <span class="shape-lab-corner shape-lab-bl">BL</span><span class="shape-lab-corner shape-lab-br">BR</span>
  <p class="shape-lab-caption">SHAPE LAB / {mode.toUpperCase()}</p>
  <h1>Four corners. One rectangle.</h1><p>{variant.name}</p>
  <p class="shape-lab-export-note">{doc.current.note}</p>
  <p>{doc.current.hits} accepted clicks · Last: {doc.current.lastTarget}</p>
  <p>Window geometry must not cut this sheet.</p>
</article>
