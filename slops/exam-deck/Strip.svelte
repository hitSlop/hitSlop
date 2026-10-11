<script lang="ts">
  import type { ForecastDay } from "./fsrs";
  import { formatDay } from "./deck";
  import { drawStrip } from "./strip";

  interface Props { days: ForecastDay[]; cap: number; width: number; height?: number }
  const { days, cap, width, height = 104 }: Props = $props();
  let host: HTMLDivElement | undefined = $state();

  const summary = $derived(days.length
    ? `Forecast: ${days.map(day => `${formatDay(day.date)} ${Math.round(day.reviews)} cards${day.overBudget ? " (budget full)" : ""}`).join(", ")}. Daily cap ${cap}.`
    : "No forecast.");

  // View only: the SVG is rebuilt from document data and discarded with the effect.
  $effect(() => {
    if (!host || width < 120 || !days.length) return;
    const svg = drawStrip(days, cap, Math.floor(width), height);
    host.replaceChildren(svg);
    return () => svg.remove();
  });
</script>

<div class="deck-strip" role="img" aria-label={summary} style:height="{height}px" bind:this={host}></div>
