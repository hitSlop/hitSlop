<script lang="ts">
  import { notebookView, sig, type Notebook } from "./lab";
  let { data, width, height }: { data: Notebook; width: number; height: number } = $props();
  const view = $derived(notebookView(data));
  const bar = $derived(data.kind === "bar");
  const left = 58, top = 30;
  const right = $derived(width - 22), bottom = $derived(height - 46);
  const xmin = $derived(Math.min(0, ...view.points.map(p => p.x)));
  const xmax = $derived(Math.max(xmin + 1, ...view.points.map(p => p.x)));
  const ymin = $derived(Math.min(0, ...view.points.map(p => p.y - (data.errorBars ? p.err : 0))));
  const ymax = $derived(Math.max(ymin + 1, ...view.points.map(p => p.y + (data.errorBars ? p.err : 0))));
  const sx = (x: number) => left + (x - xmin) / (xmax - xmin) * (right - left);
  const sy = (y: number) => bottom - (y - ymin) / (ymax - ymin) * (bottom - top);
  const slot = $derived((right - left) / Math.max(1, view.points.length));
  const line = $derived([...view.points].sort((a, b) => a.x - b.x).map((p, i) => `${i ? "L" : "M"}${sx(p.x)},${sy(p.y)}`).join(" "));
</script>

<svg {width} {height} style:display="block" style:width={`${width}px`} style:height={`${height}px`} viewBox={`0 0 ${width} ${height}`} role="img" aria-label={view.summary} font-family="monospace" font-size="11" fill="var(--slop-ink)">
  {#each [0, 1, 2, 3, 4] as i}
    {@const y = ymin + (ymax - ymin) * i / 4}
    <line x1={left} x2={right} y1={sy(y)} y2={sy(y)} stroke="var(--slop-ink)" stroke-opacity="0.15" />
    <text x={left - 9} y={sy(y) + 4} text-anchor="end">{sig(y)}</text>
    {#if !bar}{@const x = xmin + (xmax - xmin) * i / 4}<text x={sx(x)} y={bottom + 18} text-anchor="middle">{sig(x)}</text>{/if}
  {/each}
  <line x1={left} x2={right} y1={sy(0)} y2={sy(0)} stroke="var(--slop-ink)" />
  {#if data.kind === "line"}<path d={line} fill="none" stroke="var(--slop-series)" stroke-width="2.2" />{/if}
  {#if view.fit && !bar}<path d={`M${sx(xmin)},${sy(view.fit.slope * xmin + view.fit.intercept)} L${sx(xmax)},${sy(view.fit.slope * xmax + view.fit.intercept)}`} fill="none" stroke="var(--slop-fit)" stroke-width="2.4" stroke-dasharray="7 5" />{/if}
  {#each view.points as p, i}
    {@const x = bar ? left + slot * (i + 0.5) : sx(p.x)}
    {#if bar}<rect x={x - slot * 0.35} y={Math.min(sy(0), sy(p.y))} width={slot * 0.7} height={Math.abs(sy(p.y) - sy(0))} fill="var(--slop-series)" fill-opacity="0.85" /><text x={x} y={bottom + 18} text-anchor="middle" font-size="10">{p.label || p.x}</text>{:else}<circle cx={x} cy={sy(p.y)} r="4.6" fill="var(--slop-series)" stroke="var(--slop-card)" stroke-width="1.5" />{/if}
    {#if data.errorBars && p.err > 0}<path d={`M${x},${sy(p.y - p.err)}V${sy(p.y + p.err)} M${x - 4},${sy(p.y - p.err)}h8 M${x - 4},${sy(p.y + p.err)}h8`} fill="none" stroke="var(--slop-ink)" stroke-width="1.6" />{/if}
  {/each}
  <text x={left} y="16">{view.yTitle}</text><text x={(left + right) / 2} y={height - 6} text-anchor="middle">{view.xTitle}</text>
</svg>
