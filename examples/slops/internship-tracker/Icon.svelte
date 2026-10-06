<script lang="ts">
  import doc from "./schema";
  const stages = $derived(doc.current.applications.slice(0, 3).map((app) => app.stage));
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-strap"><span></span></div>
  <div class="icon-badge">
    <div class="icon-band"></div>
    <div class="icon-title"></div>
    {#each [0, 1, 2] as index}
      <div class="icon-row"><i class="icon-stamp" data-stage={stages[index] ?? "applied"}></i><i class="icon-line"></i></div>
    {/each}
  </div>
</div>
