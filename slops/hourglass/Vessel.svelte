<script lang="ts">
import { base, body, caps, height, neck, posts, repose, sandLevels, width } from "./glass";

let {
  remaining,
  running = false,
  quiet = false,
  solid = false,
  layer = "all",
}: {
  /** How much sand is still in the top bulb, 0 to 1. */
  remaining: number;
  /** Sand is falling through the neck. */
  running?: boolean;
  /** Sand recedes while the glass holds the time picker. */
  quiet?: boolean;
  /** Paint the glass itself; a window's frost does that instead. */
  solid?: boolean;
  /** The glass and sand, the walnut frame, or both. The window turns only the glass over. */
  layer?: "all" | "glass" | "frame";
} = $props();

const id = $props.id();
const levels = $derived(sandLevels(remaining));
// A falling stream dips the top surface over the neck; the pile is a cone under it.
const dip = $derived(running ? Math.min(5, (neck - levels.top) * 0.3) : 0);
const spread = $derived((base - levels.peak) / repose);
const top = $derived(
  neck - levels.top > 0.5
    ? `M0 ${levels.top - dip} Q${width / 2} ${levels.top + dip} ${width} ${levels.top - dip} V${neck} H0 Z`
    : "",
);
// Sand rounds over at the top of its heap rather than meeting in a point.
const crown = $derived(Math.min(spread * 0.3, 12));
const pile = $derived(
  base - levels.peak > 0.5
    ? `M${width / 2 - spread} ${base} L${width / 2 - crown} ${levels.peak + crown * repose} ` +
        `Q${width / 2} ${levels.peak - crown * repose * 0.4} ${width / 2 + crown} ${levels.peak + crown * repose} ` +
        `L${width / 2 + spread} ${base} Z`
    : "",
);
</script>

<svg class="hourglass-art" viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
  <defs>
    <clipPath id={`${id}-glass`}><path d={body} /></clipPath>
    <linearGradient id={`${id}-sand`} x1="0" x2="1">
      <stop offset="0.15" class="hourglass-sand-edge" />
      <stop offset="0.5" class="hourglass-sand-middle" />
      <stop offset="0.85" class="hourglass-sand-edge" />
    </linearGradient>
    <linearGradient id={`${id}-light`} x1="0" x2="0" y1="0" y2="1">
      <stop offset="0" stop-color="#fff" stop-opacity="0.32" />
      <stop offset="0.35" stop-color="#fff" stop-opacity="0" />
    </linearGradient>
    <linearGradient id={`${id}-wood`} x1="0" x2="0" y1="0" y2="1">
      <stop offset="0" stop-color="#fff" stop-opacity="0.2" />
      <stop offset="0.45" stop-color="#fff" stop-opacity="0" />
      <stop offset="1" stop-color="#000" stop-opacity="0.22" />
    </linearGradient>
    <linearGradient id={`${id}-post`} x1="0" x2="1">
      <stop offset="0" stop-color="#000" stop-opacity="0.2" />
      <stop offset="0.4" stop-color="#fff" stop-opacity="0.22" />
      <stop offset="1" stop-color="#000" stop-opacity="0.25" />
    </linearGradient>
  </defs>

  {#if layer !== "glass"}
    {#each posts as post}
      <rect class="hourglass-frame" x={post.x} y={post.y} width={post.width} height={post.height} rx="3" />
      <rect x={post.x} y={post.y} width={post.width} height={post.height} rx="3" fill={`url(#${id}-post)`} />
    {/each}
  {/if}

  {#if layer !== "frame"}
  {#if solid}<path class="hourglass-glass-solid" d={body} />{/if}

  <g class="hourglass-sand" data-quiet={quiet} clip-path={`url(#${id}-glass)`}>
    {#if top}
      <path d={top} fill={`url(#${id}-sand)`} />
      <path d={top} fill={`url(#${id}-light)`} />
    {/if}
    {#if running}
      <line class="hourglass-stream" x1={width / 2} y1={neck - 2} x2={width / 2} y2={levels.peak} />
    {/if}
    {#if pile}
      <path d={pile} fill={`url(#${id}-sand)`} />
      <path d={pile} fill={`url(#${id}-light)`} />
    {/if}
  </g>

  <path class="hourglass-rim-shade" d={body} />
  <path class="hourglass-rim" d={body} />
  <g class="hourglass-shine">
    <path d="M58 66 Q44 118 90 172" />
    <path d={`M58 ${height - 66} Q44 ${height - 118} 90 ${height - 172}`} />
    <path class="hourglass-shine-soft" d="M228 76 Q238 98 233 122" />
    <path class="hourglass-shine-soft" d={`M228 ${height - 76} Q238 ${height - 98} 233 ${height - 122}`} />
  </g>
  {/if}

  {#if layer !== "glass"}
    {#each caps as d}
      <path class="hourglass-frame" {d} />
      <path {d} fill={`url(#${id}-wood)`} />
    {/each}
  {/if}
</svg>
