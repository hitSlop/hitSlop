<!-- The pad's wire binding. Decorative; shared by the editor, the export and the icon. -->
<script lang="ts">
  let { rings = 11 }: { rings?: number } = $props();
</script>

<div class="vn-spiral" aria-hidden="true">
  {#each Array.from({ length: rings }, (_, index) => index) as index (index)}<span></span>{/each}
</div>
