<script lang="ts">
  import doc from "./schema";
  import { indexLetter } from "./shared";

  const tabLetter = $derived(indexLetter(doc.current.name));
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
