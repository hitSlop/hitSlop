<script lang="ts">
  // The drawn sample frame shown until the first photo is kept: a basil seedling on a
  // sill, grown to `growth` (0 to 1). Like a real daily photo, each day sits slightly
  // differently in the frame. Colors are the drawing's own, not the theme.
  let { growth }: { growth: number } = $props();
  const uid = $props.id();

  const nudge = $derived({ x: Math.sin(growth * 37) * 3, y: Math.cos(growth * 23) * 2 });
  const height = $derived(8 + growth * 118);
  const pairs = $derived(growth < 0.06 ? 0 : Math.min(5, 1 + Math.floor(growth * 4.6)));
  const leaves = $derived(Array.from({ length: pairs }, (_, index) => {
    const along = pairs === 1 ? 1 : 0.35 + (index / (pairs - 1)) * 0.65;
    const age = (pairs - index) / pairs; // lower pairs are older and larger
    return { y: 186 - height * along, scale: 0.35 + growth * 0.55 * (0.6 + age * 0.5), turn: index % 2 ? 8 : -6 };
  }));
  const sun = $derived(70 + growth * 60);
  const leafPath = "M0 0 C 10 -13, 34 -14, 46 -2 C 35 9, 11 10, 0 0 Z";
</script>

<svg viewBox="0 0 400 300" preserveAspectRatio="xMidYMid slice" role="img" aria-label="A drawn basil seedling (sample)">
  <defs>
    <linearGradient id={`${uid}-wall`} x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#e9dcc6" /><stop offset="1" stop-color="#cdb99b" /></linearGradient>
    <linearGradient id={`${uid}-sky`} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#bcd6e4" /><stop offset="1" stop-color="#e8eee6" /></linearGradient>
    <radialGradient id={`${uid}-sun`} cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#fff6d8" stop-opacity="0.75" /><stop offset="1" stop-color="#fff6d8" stop-opacity="0" /></radialGradient>
  </defs>
  <g transform={`translate(${nudge.x} ${nudge.y})`}>
    <rect x="-10" y="-10" width="420" height="320" fill={`url(#${uid}-wall)`} />
    <rect x="18" y="20" width="132" height="196" rx="3" fill={`url(#${uid}-sky)`} stroke="#f4ecdf" stroke-width="9" />
    <line x1="84" y1="20" x2="84" y2="216" stroke="#f4ecdf" stroke-width="6" />
    <line x1="18" y1="112" x2="150" y2="112" stroke="#f4ecdf" stroke-width="6" />
    <ellipse cx={sun} cy="232" rx="120" ry="34" fill={`url(#${uid}-sun)`} />
    <rect x="-10" y="226" width="420" height="16" fill="#efe4d2" />
    <rect x="-10" y="242" width="420" height="70" fill="#b49d7e" />
    <!-- stem and leaves -->
    <path d={`M200 186 C ${197 - growth * 4} ${186 - height * 0.5}, ${203 + growth * 3} ${186 - height * 0.8}, 200 ${186 - height}`} fill="none" stroke="#5f8f3e" stroke-width={2 + growth * 2.4} stroke-linecap="round" />
    {#each leaves as leaf, index (index)}
      <g transform={`translate(200 ${leaf.y})`}>
        <path d={leafPath} fill="#6aa84f" transform={`rotate(${-24 + leaf.turn}) scale(${leaf.scale})`} />
        <path d={leafPath} fill="#5b9a42" transform={`scale(-1 1) rotate(${-24 - leaf.turn}) scale(${leaf.scale})`} />
      </g>
    {/each}
    {#if growth >= 0.06}<ellipse cx="200" cy={186 - height} rx={3 + growth * 4} ry={2.5 + growth * 3} fill="#79b85a" />{/if}
    <!-- pot -->
    <ellipse cx="200" cy="186" rx="44" ry="8" fill="#4a3426" />
    <path d="M150 178 h100 l-4 14 h-92 z" fill="#c4683f" />
    <path d="M156 192 h88 l-10 54 h-68 z" fill="#b65b35" />
    <path d="M170 192 h10 l-5 54 h-6 z" fill="#d07a50" opacity="0.6" />
  </g>
</svg>

<style>
  svg { display: block; width: 100%; height: 100%; }
</style>
