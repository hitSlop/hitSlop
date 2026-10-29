<script lang="ts">
  import { ringArc, type Person, type Totals } from "./chores";

  let { people, totals, size = 168 }: { people: readonly Person[]; totals: readonly Totals[]; size?: number } = $props();

  const total = $derived(totals.reduce((sum, t) => sum + t.assigned, 0));
  const doneTotal = $derived(totals.reduce((sum, t) => sum + t.done, 0));
  const gap = 0.06;
  const segments = $derived.by(() => {
    let angle = 0;
    return people.flatMap((person, index) => {
      const t = totals[index];
      if (!t || !t.assigned || !total) return [];
      const span = (t.assigned / total) * Math.PI * 2;
      const from = angle + gap / 2;
      const to = angle + span - gap / 2;
      angle += span;
      if (to <= from) return [];
      const doneTo = from + (to - from) * Math.min(1, t.done / t.assigned);
      return [{ id: person.$id, tone: person.tone, outer: ringArc(84, 84, 80, 56, from, to), inner: t.done ? ringArc(84, 84, 54, 43, from, doneTo) : "" }];
    });
  });
</script>

<svg class="wheel" viewBox="0 0 168 168" width={size} height={size} role="img" aria-label={`${doneTotal} of ${total} points done this week`}>
  <circle cx="84" cy="84" r="68" class="wheel-track" fill="var(--tiny-paper)" stroke="var(--tiny-rule)" stroke-width="1" />
  {#each segments as segment (segment.id)}
    <g data-tone={segment.tone}>
      <path d={segment.outer} class="wheel-assigned" fill={`var(--tiny-${segment.tone})`} fill-opacity="0.3" />
      {#if segment.inner}<path d={segment.inner} class="wheel-done" fill={`var(--tiny-${segment.tone})`} />{/if}
    </g>
  {/each}
  <text x="84" y="80" class="wheel-count" text-anchor="middle" dominant-baseline="central" fill="var(--tiny-ink)" font-family="sans-serif" font-size="30" font-weight="800">{doneTotal}<tspan class="wheel-of" font-size="16" fill="var(--tiny-muted)">/{total}</tspan></text>
  <text x="84" y="100" class="wheel-label" text-anchor="middle" fill="var(--tiny-muted)" font-family="sans-serif" font-size="8" font-weight="700">points done</text>
</svg>
