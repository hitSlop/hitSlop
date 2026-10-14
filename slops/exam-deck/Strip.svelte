<script lang="ts">
  import type { ForecastDay } from "./fsrs";
  import { capInScale } from "./strip";
  import { dateLabel } from "../_shared/format";
  let { days, cap, width, height = 104 }: { days: ForecastDay[]; cap: number; width: number; height?: number } = $props();
  const capShown = $derived(capInScale(days, cap));
  const max = $derived(Math.max(capShown ? cap : 0, ...days.map(day => day.reviews), 4) * 1.22);
  const plotHeight = $derived(height - 40);
  const slot = $derived((width - 36) / Math.max(1, days.length));
  const y = (value: number) => height - 25 - value / max * plotHeight;
</script>
<svg {width} {height} style:display="block" style:width={`${width}px`} style:height={`${height}px`} viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Forecast cards per day" font-family="sans-serif" font-size="10" fill="var(--slop-graphite)">
  <line x1="0" x2={width - 30} y1={y(0)} y2={y(0)} stroke="var(--slop-form)" />
  {#each days as day, i}
    {@const x = 4 + i * slot}
    <rect {x} y={y(day.reviews)} width={slot * 0.75} height={Math.max(0, day.reviews - day.newCards) / max * plotHeight} fill="var(--slop-graphite)" />
    <rect {x} y={y(day.newCards)} width={slot * 0.75} height={day.newCards / max * plotHeight} fill="var(--slop-tint)" stroke="var(--slop-form)" stroke-width="0.5" />
    <text x={x + slot * 0.375} y={y(day.reviews) - 5} text-anchor="middle">{Math.round(day.reviews)}{day.overBudget ? "!" : ""}</text>
    <text x={x + slot * 0.375} y={height - 10} text-anchor="middle">{dateLabel(day.date, { weekday: "narrow" })}</text>
    <text x={x + slot * 0.375} y={height} text-anchor="middle">{Number(day.date.slice(8))}</text>
  {/each}
  {#if capShown}<line x1="0" x2={width - 30} y1={y(cap)} y2={y(cap)} stroke="var(--slop-form)" stroke-dasharray="4 3" /><text x={width - 4} y={y(cap) + 4} text-anchor="end">{cap}</text>{/if}
</svg>
