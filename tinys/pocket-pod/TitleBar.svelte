<script lang="ts">
let { title, playing = false, paused = false, held = false }: {
  title: string;
  playing?: boolean;
  paused?: boolean;
  held?: boolean;
} = $props();
</script>

<header class="pod-bar">
  <strong class="pod-bar-title">{title}</strong>
  {#if held}
    <svg class="pod-bar-glyph" viewBox="0 0 12 14" role="img" aria-label="Hold is on"><rect x="1" y="6" width="10" height="7" rx="1.5" /><path d="M3.5 6V4a2.5 2.5 0 0 1 5 0v2" fill="none" /></svg>
  {/if}
  {#if playing}
    <svg class="pod-bar-glyph" viewBox="0 0 12 14" role="img" aria-label="Playing"><path d="M2 1l9 6-9 6z" /></svg>
  {:else if paused}
    <svg class="pod-bar-glyph" viewBox="0 0 12 14" role="img" aria-label="Paused"><path d="M2 1h3v12H2zM7 1h3v12H7z" /></svg>
  {/if}
  <svg class="pod-battery" viewBox="0 0 26 12" aria-hidden="true">
    <rect x="0.75" y="0.75" width="21.5" height="10.5" rx="2" fill="none" />
    <rect x="23" y="3.5" width="2.5" height="5" rx="1" />
    <rect x="2.5" y="2.5" width="18" height="7" rx="1" class="pod-battery-fill" />
  </svg>
</header>
