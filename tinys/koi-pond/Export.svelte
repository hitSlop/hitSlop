<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { pondView } from "./shared";
  let { snapshot }: CaptureProps<typeof schema> = $props();
  const view = $derived(pondView(snapshot));
  const position = (index: number) => {
    const angle = (index * 2.39996 + snapshot.seed * 0.17) % (Math.PI * 2);
    const radius = 45 + index % 3 * 35;
    return { x: 210 + Math.cos(angle) * radius, y: 210 + Math.sin(angle) * radius, turn: angle * 180 / Math.PI + 80 };
  };
</script>
<figure class="koi-export">
  <svg width="420" height="420" viewBox="0 0 420 420" role="img" aria-label={`Koi pond with ${view.count ? view.names : "no koi"}`}>
    <defs><radialGradient id="pond-water"><stop stop-color="var(--tiny-shallow)"/><stop offset="1" stop-color="var(--tiny-deep)"/></radialGradient></defs>
    <circle cx="210" cy="210" r="198" fill="var(--tiny-stone)"/>
    <circle cx="210" cy="210" r="184" fill="url(#pond-water)"/>
    {#each [60, 105, 150] as radius}<circle cx="210" cy="210" r={radius} fill="none" stroke="var(--tiny-paper)" stroke-opacity="0.12" stroke-width="2"/>{/each}
    {#each snapshot.koi as koi, index}
      {@const at = position(index)}
      <g transform={`translate(${at.x} ${at.y}) rotate(${at.turn}) scale(${koi.size})`}>
        <ellipse cx="3" cy="6" rx="15" ry="38" fill="var(--tiny-deep)" fill-opacity="0.25"/>
        <path d="M0 26 L-15 47 L0 42 L15 47Z" fill={koi.pattern === "ogon" ? "var(--tiny-gold)" : "var(--tiny-cream)"}/>
        <path d="M-8 -4L-23 13L-10 14 M8 -4L23 13L10 14" fill="var(--tiny-cream)" fill-opacity="0.8"/>
        <ellipse cy="-4" rx="13" ry="34" fill={koi.pattern === "ogon" ? "var(--tiny-gold)" : koi.pattern === "asagi" ? "var(--tiny-slate)" : "var(--tiny-cream)"}/>
        {#if koi.pattern === "tancho"}<ellipse cy="-26" rx="9" ry="9" fill="var(--tiny-red)"/>{:else if koi.pattern !== "ogon"}<ellipse cx="-3" cy="-13" rx="9" ry="12" fill="var(--tiny-red)"/><ellipse cx="4" cy="12" rx="7" ry="10" fill={koi.pattern === "showa" ? "var(--tiny-charcoal)" : "var(--tiny-orange)"}/>{/if}
        <circle cx="-8" cy="-27" r="1.7" fill="var(--tiny-charcoal)"/><circle cx="8" cy="-27" r="1.7" fill="var(--tiny-charcoal)"/>
      </g>
    {/each}
    <path d="M88 94L120 81A37 37 0 1 0 115 119Z" fill="var(--tiny-lily)"/><path d="M320 306L342 291A30 30 0 1 0 340 329Z" fill="var(--tiny-lily)"/>
    <g transform="translate(94 96)"><ellipse rx="8" ry="18" fill="var(--tiny-lotus)"/><ellipse rx="18" ry="8" fill="var(--tiny-lotus)"/><circle r="6" fill="var(--tiny-gold)"/></g>
  </svg>
  <figcaption><strong>Koi Pond</strong><span>{view.count ? view.names : "A quiet pond"} · {view.feedingLabel}</span></figcaption>
</figure>
