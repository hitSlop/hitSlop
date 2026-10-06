<script lang="ts">
  import { Html, Svg, getLayerCakeContext } from "layercake";
  import { sankeyLinkHorizontal } from "d3-sankey";
  import { buildFlow, type FlowLink, type FlowNode, type PlacedLink, type PlacedNode } from "./flow";
  import { money, share } from "./money";
  import type { Plan } from "./schema";

  interface Props { plan: Pick<Plan, "sources" | "buckets" | "currency">; interactive?: boolean }
  let { plan, interactive = false }: Props = $props();

  // Layer Cake measures the box; d3-sankey places the nodes inside it.
  const k = getLayerCakeContext();
  const flow = $derived(buildFlow(plan, k.width, k.height));
  const path = sankeyLinkHorizontal<FlowNode, FlowLink>();
  let hovered = $state<string | null>(null);

  const from = (link: PlacedLink) => link.source as PlacedNode;
  const to = (link: PlacedLink) => link.target as PlacedNode;
  const lit = (link: PlacedLink) => !hovered || link.id === hovered || from(link).id === hovered || to(link).id === hovered;
  const readout = $derived.by(() => {
    const link = flow.links.find((candidate) => candidate.id === hovered || from(candidate).id === hovered || to(candidate).id === hovered);
    if (!hovered || !link) return "";
    const node = from(link).id === "hub" ? to(link) : from(link);
    return `${node.name}: ${money(node.value, plan.currency)} · ${share(node.value, Math.max(flow.income, flow.spend))} of the total`;
  });
  const on = (id: string) => interactive ? { onpointerenter: () => (hovered = id), onpointerleave: () => (hovered = null) } : {};
</script>

<Svg label="Where this month's money goes">
  {#each flow.links as link (link.id)}
    <path d={path(link)} fill="none" style:stroke={link.color} stroke-width={Math.max(1, link.width ?? 1)} opacity={lit(link) ? 0.5 : 0.12} {...on(link.id)}>
      <title>{from(link).name} → {to(link).name}: {money(link.value, plan.currency)}</title>
    </path>
  {/each}
  {#each flow.nodes as node (node.id)}
    <rect x={node.x0} y={node.y0} width={(node.x1 ?? 0) - (node.x0 ?? 0)} height={Math.max(2, (node.y1 ?? 0) - (node.y0 ?? 0))} rx="3" style:fill={node.color} {...on(node.id)} />
  {/each}
</Svg>

<Html>
  {#each flow.nodes as node (node.id)}
    {#if node.id !== "hub"}
      {@const isSource = (node.sourceLinks?.length ?? 0) > 0 && node.id !== "left"}
      <div
        class="label"
        data-side={isSource ? "in" : "out"}
        style:top="{((node.y0 ?? 0) + (node.y1 ?? 0)) / 2}px"
        style:left={isSource ? undefined : `${(node.x1 ?? 0) + 8}px`}
        style:right={isSource ? `${k.width - (node.x0 ?? 0) + 8}px` : undefined}
      ><b>{node.name}</b><span>{money(node.value, plan.currency)}</span></div>
    {/if}
  {/each}
  {#if interactive && readout}<p class="readout" role="status">{readout}</p>{/if}
</Html>
