<script lang="ts">
  import doc from "./schema";
  const shade = $derived(doc.current.products[0]?.shade ?? "#e8a0a8");
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-compact">
    <div class="icon-glass"></div>
    <div class="icon-pan" style:--shade={shade}></div>
  </div>
</div>
