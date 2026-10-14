<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const open = $derived(snapshot.items.slice(0, 6).map((item) => item.revealed));
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
