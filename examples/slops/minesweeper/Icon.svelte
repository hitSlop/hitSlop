<div class="icon-surface" aria-hidden="true">
  <div class="icon-chart">
    {#each [0, 1, 2, 3, 4, 5, 6, 7, 8] as index}
      <i class="icon-cell" data-open={index !== 4 && index !== 8} data-flag={index === 4}>{#if index === 4}⚑{:else if index === 1 || index === 3}1{:else if index === 5}2{/if}</i>
    {/each}
  </div>
</div>
