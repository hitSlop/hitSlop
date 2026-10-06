<div class="icon-surface" aria-hidden="true">
  <div class="icon-board">
    {#each ["GO", "NOW", "OK"] as word}
      <p>{#each [...word.padEnd(4)] as char}<i>{char}</i>{/each}</p>
    {/each}
  </div>
</div>
