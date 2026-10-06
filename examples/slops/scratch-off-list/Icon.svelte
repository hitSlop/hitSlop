<script lang="ts">
  import doc from "./schema";
  const open = $derived(doc.current.items.slice(0, 6).map((item) => item.revealed));
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-ticket">
    <div class="icon-band"></div>
    <div class="icon-grid">
      {#each [0, 1, 2, 3, 4, 5] as index}
        <i class="icon-tile" data-open={open[index] ?? index < 2}>{#if open[index] ?? index < 2}★{:else}?{/if}</i>
      {/each}
    </div>
  </div>
</div>
