<script lang="ts">
import doc from "./schema";
  import { spriteView } from "./sprite";
  const { cells } = $derived(spriteView(doc.current));
</script>

<article class="exportSheet" aria-label="Exported 16 by 16 sprite">
    <div class="exportBezel">
      <div class="exportGrid">
        {#each cells as color, index (index)}
          <span class="cell" style:background={color || "transparent"}></span>
        {/each}
      </div>
    </div>
  </article>
