<script lang="ts">
  // The answer sheet in miniature: timing marks, the printed header, a filled answer bubble
  // and a short load strip. It is drawn with plain elements so it stays crisp at icon size.
  const bars = [0.9, 0.62, 0.74, 0.46, 0.55, 0.34, 0.4];
</script>

<div class="deck-icon" aria-hidden="true">
  <div class="deck-icon-sheet">
    <div class="deck-icon-timing"></div>
    <div class="deck-icon-head"><span></span></div>
    <div class="deck-icon-bubbles">
      {#each [1, 2, 3, 4] as n (n)}<span data-filled={n === 3}>{n}</span>{/each}
    </div>
    <div class="deck-icon-bars">
      {#each bars as height, i (i)}<i style:height="{height * 100}%"></i>{/each}
    </div>
  </div>
</div>
