<script lang="ts">
  import type { Product } from "./schema";
  import { hole } from "./pan";

  let { product, selected = false, onselect }: { product: Product; selected?: boolean; onselect?: () => void } = $props();
</script>

{#snippet dish()}
  <span class="dish" style:--shade={product.shade} style:--hole={hole(product.left)} data-empty={product.left === 0 ? "" : undefined}></span>
{/snippet}

{#if onselect}
  <button class="pan" aria-pressed={selected} aria-label={`${product.name || "Unnamed"}, ${product.left}% left`} onclick={onselect}>
    {@render dish()}
    <span class="pan-name">{product.name || "Unnamed"}</span>
    <span class="pan-left">{product.left === 0 ? "panned" : `${product.left}%`}</span>
  </button>
{:else}
  <div class="pan">
    {@render dish()}
    <span class="pan-name">{product.name || "Unnamed"}</span>
    <span class="pan-left">{product.left === 0 ? "panned" : `${product.left}%`}</span>
  </div>
{/if}
