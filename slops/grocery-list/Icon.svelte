<script lang="ts">
  import doc from "./schema";

  const doneCount = $derived(doc.current.items.filter((item) => item.done).length);
  const totalCount = $derived(doc.current.items.length);
  const marks = $derived(totalCount > 0 ? Math.round(4 * doneCount / totalCount) : 0);
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-plate">
    <div class="icon-pad">
      <div class="icon-title"></div>
      <div class="icon-lines">
        {#each [0, 1, 2, 3] as index}
          <div class="icon-row">
            <span class="icon-box" data-done={index < marks}></span>
            <i class="icon-line"></i>
          </div>
        {/each}
      </div>
    </div>
    <div class="icon-banana"></div>
    <span class="icon-star">★</span>
  </div>
</div>
