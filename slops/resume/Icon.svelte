<script lang="ts">
  import doc from "./schema";
  import { resumeView } from "./entries";

  const { initials } = $derived(resumeView(doc.current));
</script>

<div class="iconSurface" aria-hidden="true">
  <article class="iconSheet">
    <div class="iconRail"></div>
    <div class="iconMonogram">{initials}</div>
  </article>
</div>
