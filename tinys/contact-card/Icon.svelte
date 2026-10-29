<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { indexLetter } from "./shared";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const tabLetter = $derived(indexLetter(snapshot.name));
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconPlate">
    <div class="iconBadge">
      <div class="iconNotch"></div>
      <span class="iconTab">{tabLetter}</span>
      <div class="iconAvatar">{tabLetter}</div>
      <div class="iconTextGroup">
        <span class="iconNameBar"></span>
        <span class="iconSubBar"></span>
      </div>
      <div class="iconLines">
        <span class="iconRow"></span>
        <span class="iconRow"></span>
        <span class="iconRow"></span>
      </div>
      <div class="iconButtons">
        <span class="iconCircle"></span>
        <span class="iconCircle"></span>
        <span class="iconCircle"></span>
      </div>
    </div>
  </div>
</div>
