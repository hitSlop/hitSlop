<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
</script>

<svg class="lab-icon" viewBox="0 0 400 400" aria-hidden="true">
  <rect width="400" height="400" fill="var(--tiny-paper)" />
  <rect x="44" y="44" width="312" height="312" rx="10" fill="var(--tiny-card)" />
  {#each [96, 148, 200, 252, 304] as v}
    <path d="M{v} 56V344M56 {v}H344" stroke="var(--tiny-grid)" stroke-width="3" />
  {/each}
  <path d="M56 344V56M56 344H344" stroke="var(--tiny-ink)" stroke-width="7" stroke-linecap="round" fill="none" />
  <path d="M82 300L330 108" stroke="var(--tiny-fit)" stroke-width="9" stroke-dasharray="22 14" stroke-linecap="round" />
  {#each [[104, 286, 22], [152, 258, 18], [204, 232, 24], [254, 176, 16], [302, 134, 20]] as [x, y, e]}
    <path d="M{x} {y - e}V{y + e}M{x - 9} {y - e}H{x + 9}M{x - 9} {y + e}H{x + 9}" stroke="var(--tiny-ink)" stroke-width="5" stroke-linecap="round" />
    <circle cx={x} cy={y} r="13" fill="var(--tiny-series)" stroke="var(--tiny-card)" stroke-width="4" />
  {/each}
</svg>
