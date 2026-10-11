<script lang="ts">
  import Monster from "./Monster.svelte";
</script>

<div class="sq-icon" aria-hidden="true">
  <div class="sq-icon-map">
    <svg viewBox="0 0 400 400">
      <path class="sq-icon-trail" d="M40 330 C120 330 90 230 170 230 C250 230 220 130 300 120" />
      <circle cx="40" cy="330" r="16" fill="var(--slop-mint)" stroke="var(--slop-ink)" stroke-width="7" />
      <circle cx="170" cy="230" r="16" fill="var(--slop-pink)" stroke="var(--slop-ink)" stroke-width="7" />
    </svg>
    <div class="sq-icon-boss"><Monster color="var(--slop-boss)" ko={false} seed={0} /></div>
    <span class="sq-icon-star">⭐</span>
  </div>
</div>
