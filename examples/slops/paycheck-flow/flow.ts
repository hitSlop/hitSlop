import { sankey, sankeyJustify, type SankeyLink, type SankeyNode } from "d3-sankey";
import type { Plan } from "./schema";

export interface FlowNode { id: string; name: string; color: string; value: number }
export interface FlowLink { id: string; color: string }
export type PlacedNode = SankeyNode<FlowNode, FlowLink>;
export type PlacedLink = SankeyLink<FlowNode, FlowLink>;

export interface Flow {
  nodes: PlacedNode[];
  links: PlacedLink[];
  income: number;
  spend: number;
  /** Income minus spending: positive is left over, negative is over budget. */
  gap: number;
}

/** Money in, one hub, money out. A shortfall enters as its own source so every ribbon balances. */
export function buildFlow(plan: Pick<Plan, "sources" | "buckets">, width: number, height: number): Flow {
  const sources = plan.sources.filter((source) => source.amount > 0);
  const buckets = plan.buckets.filter((bucket) => bucket.amount > 0);
  const income = sources.reduce((sum, source) => sum + source.amount, 0);
  const spend = buckets.reduce((sum, bucket) => sum + bucket.amount, 0);
  const gap = income - spend;

  const nodes: FlowNode[] = [];
  const links: { source: string; target: string; value: number; id: string; color: string }[] = [];
  const hub: FlowNode = { id: "hub", name: "Total", color: "var(--slop-hub)", value: Math.max(income, spend) };

  sources.forEach((source, index) => {
    const id = `in${index}`;
    nodes.push({ id, name: source.name || "Income", color: "var(--slop-income)", value: source.amount });
    links.push({ source: id, target: "hub", value: source.amount, id: `l-${id}`, color: "var(--slop-income)" });
  });
  if (gap < 0) {
    nodes.push({ id: "short", name: "Over budget", color: "var(--slop-warn)", value: -gap });
    links.push({ source: "short", target: "hub", value: -gap, id: "l-short", color: "var(--slop-warn)" });
  }
  nodes.push(hub);
  buckets.forEach((bucket, index) => {
    const id = `out${index}`;
    const color = `var(--slop-${bucket.tone})`;
    nodes.push({ id, name: bucket.name || "Spending", color, value: bucket.amount });
    links.push({ source: "hub", target: id, value: bucket.amount, id: `l-${id}`, color });
  });
  if (gap > 0) {
    nodes.push({ id: "left", name: "Left over", color: "var(--slop-gold)", value: gap });
    links.push({ source: "hub", target: "left", value: gap, id: "l-left", color: "var(--slop-gold)" });
  }

  if (!links.length || width < 20 || height < 20) return { nodes: [], links: [], income, spend, gap };
  const layout = sankey<FlowNode, FlowLink>()
    .nodeId((node) => node.id)
    .nodeWidth(14)
    .nodePadding(16)
    .nodeAlign(sankeyJustify)
    .nodeSort(null)
    .linkSort(null)
    .extent([[0, 0], [width, height]]);
  const graph = layout({ nodes: nodes.map((node) => ({ ...node })), links: links.map((link) => ({ ...link })) });
  return { nodes: graph.nodes, links: graph.links, income, spend, gap };
}
