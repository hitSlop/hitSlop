<script lang="ts">
  import doc from "./schema";
  import { badgeInitials } from "./schema";

  const initials = $derived(badgeInitials(doc.current.name, doc.current.initials));
</script>

<div class="iconSurface" aria-hidden="true">
  <article class="iconSheet">
    <div class="iconRail"></div>
    <div class="iconMonogram">{initials}</div>
  </article>
</div>
