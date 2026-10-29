<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-board">
    {#each ["GO", "NOW", "OK"] as word}
      <p>{#each [...word.padEnd(4, "\u00a0")] as char}<i>{char}</i>{/each}</p>
    {/each}
  </div>
</div>
