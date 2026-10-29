<script lang="ts">
  // The graph drawn as plain SVG from the same window and y-range the live board uses:
  // capture-safe (no library, no async) for the export page.
  import type { Params, Plot } from "./latex";
  import { curvePath, tickStep, yRange } from "./notebook";

  let { plot, params, xMin, xMax, width = 400, height = 200, label }: {
    plot: Plot | null; params: Params; xMin: number; xMax: number; width?: number; height?: number; label: string;
  } = $props();

  const y = $derived(yRange(plot, params, xMin, xMax));
  const sx = (x: number) => (x - xMin) / (xMax - xMin) * width;
  const sy = (v: number) => height - (v - y[0]) / (y[1] - y[0]) * height;
  function ticks(lo: number, hi: number) {
    const step = tickStep(hi - lo, 8), out: number[] = [];
    for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) out.push(Math.round(v / step) * step);
    return out;
  }
  const tickLabel = (v: number) => String(Math.round(v * 1000) / 1000).replace("-", "−");
  const xs = $derived(ticks(xMin, xMax));
  const ys = $derived(ticks(y[0], y[1]));
  const path = $derived(curvePath(plot, params, xMin, xMax, y, width, height));
  const axisX = $derived(Math.max(0, Math.min(height, sy(0))));
  const axisY = $derived(Math.max(0, Math.min(width, sx(0))));
</script>

<svg class="plot-figure" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={label}>
  <rect width={width} height={height} class="plot-paper" fill="var(--tiny-plotPaper)" />
  {#each xs as v (v)}<line x1={sx(v)} x2={sx(v)} y1="0" y2={height} class="plot-grid" stroke="var(--tiny-grid)" stroke-width="1" />{/each}
  {#each ys as v (v)}<line y1={sy(v)} y2={sy(v)} x1="0" x2={width} class="plot-grid" stroke="var(--tiny-grid)" stroke-width="1" />{/each}
  <line x1="0" x2={width} y1={axisX} y2={axisX} class="plot-axis" stroke="var(--tiny-ink)" stroke-width="1.2" />
  <line y1="0" y2={height} x1={axisY} x2={axisY} class="plot-axis" stroke="var(--tiny-ink)" stroke-width="1.2" />
  {#each xs as v (v)}{#if Math.abs(v) > 1e-9 && sx(v) > 10 && sx(v) < width - 10}<text x={sx(v)} y={Math.min(height - 4, axisX + 13)} text-anchor="middle" class="plot-label" fill="var(--tiny-muted)" font-size="10" font-family="serif">{tickLabel(v)}</text>{/if}{/each}
  {#each ys as v (v)}{#if Math.abs(v) > 1e-9 && sy(v) > 8 && sy(v) < height - 4}<text x={Math.max(4, axisY - 5)} y={sy(v) + 4} text-anchor={axisY - 5 < 4 ? "start" : "end"} class="plot-label" fill="var(--tiny-muted)" font-size="10" font-family="serif">{tickLabel(v)}</text>{/if}{/each}
  {#if path}<path d={path} class="plot-curve" fill="none" stroke="var(--tiny-curve)" stroke-width="2.4" stroke-linejoin="round" />{/if}
</svg>
