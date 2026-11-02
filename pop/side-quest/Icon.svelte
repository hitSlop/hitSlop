<script lang="ts">
  import "@hitslop/pop/base.css";
  import "./styles.css";
  import Monster from "./Monster.svelte";
</script>

<div class="sq-icon" aria-hidden="true">
  <div class="sq-icon-map">
    <svg viewBox="0 0 400 400">
      <path class="sq-icon-trail" fill="none" stroke="var(--pop-pen)" stroke-width="14" stroke-linecap="round" stroke-dasharray="2 30" d="M40 330 C120 330 90 230 170 230 C250 230 220 130 300 120" />
      <circle cx="40" cy="330" r="16" fill="var(--pop-mint)" stroke="var(--pop-ink)" stroke-width="7" />
      <circle cx="170" cy="230" r="16" fill="var(--pop-pink)" stroke="var(--pop-ink)" stroke-width="7" />
    </svg>
    <div class="sq-icon-boss"><Monster color="var(--pop-boss)" ko={false} seed={0} /></div>
    <span class="sq-icon-star">⭐</span>
  </div>
</div>
