<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import schema from "./schema";
const doc = useDocument(schema);
</script>

<article class="exportSheet" aria-label="Exported 16 by 16 sprite">
    <div class="exportBezel">
      <div class="exportGrid">
        {#each doc.current.pixels as color, index (index)}
          <span class="cell" style:background={color || "transparent"}></span>
        {/each}
      </div>
    </div>
  </article>
