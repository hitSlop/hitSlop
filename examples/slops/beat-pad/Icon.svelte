<div class="icon-surface" aria-hidden="true">
  <div class="icon-grid">
    {#each ["10010010", "00100010", "10101010", "10000100"] as row, r}
      <p>{#each [...row] as on}<i data-on={on === "1"} data-row={r}></i>{/each}</p>
    {/each}
  </div>
</div>
