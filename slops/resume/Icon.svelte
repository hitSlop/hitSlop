<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { resumeView } from "./entries";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const { initials } = $derived(resumeView(snapshot));
</script>

<div class="iconSurface" aria-hidden="true">
  <article class="iconSheet">
    <div class="iconRail"></div>
    <div class="iconMonogram">{initials}</div>
  </article>
</div>
