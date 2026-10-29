<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const shade = $derived(snapshot.products[0]?.shade ?? "#e8a0a8");
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-compact">
    <div class="icon-glass"></div>
    <div class="icon-pan" style:--shade={shade}></div>
  </div>
</div>
