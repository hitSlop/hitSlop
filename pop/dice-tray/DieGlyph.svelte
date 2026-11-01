<script lang="ts">
  import type { Kind } from "./schema";

  // A flat drawing of a die, for the rack, the icon and the no-WebGL fallback.
  let { kind, value, size = 40 }: { kind: Kind; value?: number; size?: number } = $props();

  const outline: Record<Kind, string> = {
    d4: "50,7 95,88 5,88",
    d6: "",
    d8: "50,4 93,50 50,96 7,50",
    d10: "50,4 94,45 50,96 6,45",
    d12: "50,4 95,37 78,93 22,93 5,37",
    d20: "50,3 92,27 92,73 50,97 8,73 8,27",
  };
  const facets: Record<Kind, string> = {
    d4: "",
    d6: "",
    d8: "M7 50H93",
    d10: "M6 45L50 62L94 45M50 62V96",
    d12: "M50 20L80 42L68 78H32L20 42Z M50 4V20 M95 37L80 42 M78 93L68 78 M22 93L32 78 M5 37L20 42",
    d20: "M50 26L78 70H22Z M50 3V26 M92 27L78 70 M8 27L22 70 M50 26L8 27 M50 26L92 27 M22 70L50 97L78 70 M92 73L78 70 M8 73L22 70",
  };
  const textY: Record<Kind, number> = { d4: 70, d6: 52, d8: 52, d10: 45, d12: 52, d20: 54 };
  const label = $derived(value === undefined ? "" : String(value));
</script>

<svg class="die-glyph" viewBox="0 0 100 100" width={size} height={size} aria-hidden="true">
  {#if kind === "d6"}
    <rect class="die-glyph-body" x="10" y="10" width="80" height="80" rx="14" />
  {:else}
    <polygon class="die-glyph-body" points={outline[kind]} />
  {/if}
  {#if facets[kind]}<path class="die-glyph-facet" d={facets[kind]} />{/if}
  {#if label}<text class="die-glyph-text" x="50" y={textY[kind]} font-size={label.length > 1 ? 28 : 32}>{label}</text>{/if}
</svg>
