<script lang="ts">
  import { sankeyLinkHorizontal } from "d3-sankey";
  import { buildFlow, type FlowNode, type FlowLink } from "./flow";
  import { money } from "./money";
  import type { Plan } from "./plan";
  let { plan, width = 528, height = 320 }: { plan: Pick<Plan, "sources" | "buckets" | "currency">; width?: number; height?: number } = $props();
  const flow = $derived(buildFlow(plan, width - 236, height - 28));
  const path = sankeyLinkHorizontal<FlowNode, FlowLink>();
</script>
<svg {width} {height} viewBox={`0 0 ${width} ${height}`} aria-label="Where this month's money goes" role="img" font-family="sans-serif" font-size="11" fill="var(--slop-ink)">
  <g transform="translate(118 14)">
    {#each flow.links as link}<path d={path(link) ?? ""} fill="none" stroke={link.color} stroke-width={Math.max(1, link.width ?? 1)} opacity="0.5" />{/each}
    {#each flow.nodes as node}
      <rect x={node.x0} y={node.y0} width={(node.x1 ?? 0) - (node.x0 ?? 0)} height={Math.max(2, (node.y1 ?? 0) - (node.y0 ?? 0))} rx="3" fill={node.color} />
      {#if node.id !== "hub"}
        {@const source = (node.sourceLinks?.length ?? 0) > 0 && node.id !== "left"}
        {@const x = source ? (node.x0 ?? 0) - 8 : (node.x1 ?? 0) + 8}
        {@const y = ((node.y0 ?? 0) + (node.y1 ?? 0)) / 2}
        <text {x} y={y - 3} text-anchor={source ? "end" : "start"} font-weight="700">{node.name}</text>
        <text {x} y={y + 12} text-anchor={source ? "end" : "start"}>{money(node.value, plan.currency)}</text>
      {/if}
    {/each}
  </g>
</svg>
