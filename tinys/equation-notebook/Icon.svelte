<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./mathlive-static.css";
  import "./styles.css";
</script>

<!-- A corner of the pad: green engineering grid, the margin rule, a numbered line and its red curve. -->
<svg class="pad-icon" viewBox="0 0 400 400" aria-hidden="true">
  <defs>
    <pattern id="pad-icon-grid" width="25" height="25" patternUnits="userSpaceOnUse">
      <path d="M25 0V25H0" fill="none" stroke="var(--tiny-grid)" stroke-width="2" />
    </pattern>
  </defs>
  <rect width="400" height="400" fill="var(--tiny-paper)" />
  <rect width="400" height="400" fill="url(#pad-icon-grid)" />
  <rect width="400" height="78" fill="var(--tiny-header)" />
  <line x1="0" y1="78" x2="400" y2="78" stroke="var(--tiny-pencil)" stroke-width="5" />
  <line x1="62" y1="78" x2="62" y2="400" stroke="var(--tiny-margin)" stroke-width="4" />
  <rect x="84" y="104" width="226" height="52" rx="6" fill="var(--tiny-highlight)" />
  <text x="96" y="143" class="pad-icon-math" font-family="serif" font-style="italic" font-size="38" fill="var(--tiny-ink)">y = x² − 4</text>
  <text x="318" y="143" class="pad-icon-math pad-icon-number" font-family="serif" font-size="30" fill="var(--tiny-muted)">(1)</text>
  <rect x="84" y="180" width="288" height="190" fill="var(--tiny-plotPaper)" stroke="var(--tiny-grid)" stroke-width="3" />
  <line x1="84" y1="290" x2="372" y2="290" stroke="var(--tiny-ink)" stroke-width="3" />
  <line x1="228" y1="180" x2="228" y2="370" stroke="var(--tiny-ink)" stroke-width="3" />
  <path d="M112 196Q228 448 344 196" fill="none" stroke="var(--tiny-curve)" stroke-width="9" stroke-linecap="round" />
  <circle cx="170" cy="290" r="9" fill="var(--tiny-curve)" />
  <circle cx="286" cy="290" r="9" fill="var(--tiny-curve)" />
</svg>
