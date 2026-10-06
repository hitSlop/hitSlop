<script lang="ts">
  import doc from "./schema";
</script>

<div class="pom-icon" aria-hidden="true">
  <article class="pom-shell">
    {@render leaf()}
    <div class="pom-icon-face"><span class="pom-icon-digits">{doc.current.focusMinutes}</span><i class="pom-icon-tick"></i></div>
    <div class="pom-icon-button"></div>
  </article>
</div>

{#snippet leaf()}
  <div class="pom-leaf" aria-hidden="true"><i class="pom-leaf-blade"></i><i class="pom-leaf-blade"></i><i class="pom-leaf-blade"></i><b class="pom-leaf-stem"></b></div>
{/snippet}
