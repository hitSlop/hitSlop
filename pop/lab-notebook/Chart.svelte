<script lang="ts">
  import { drawFigure } from "./figure";
  import { notebookView, type Notebook } from "./lab";

  interface Props { data: Notebook; width: number; height: number }
  const { data, width, height }: Props = $props();
  const summary = $derived(notebookView(data).summary);
  let host: HTMLDivElement | undefined = $state();

  // View only: the SVG is rebuilt from document data and discarded with the effect.
  $effect(() => {
    if (!host || width < 40 || height < 40) return;
    const svg = drawFigure(data, Math.floor(width), Math.floor(height));
    host.replaceChildren(svg);
    return () => svg.remove();
  });
</script>

<div class="lab-chart" role="img" aria-label={summary} style:width="{width}px" style:height="{height}px" bind:this={host}></div>
