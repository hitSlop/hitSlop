<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { packingView } from "./items";
  import { iconRows } from "./shared";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const { total: totalCount, packed: packedCount, complete } = $derived(packingView(snapshot));
  const marks = $derived(totalCount > 0 ? Math.round(4 * packedCount / totalCount) : 0);
</script>

<div class="icon-surface" aria-hidden="true">
  <div class="icon-tag">
    <div class="icon-grommet">
      <div class="icon-loop"></div>
      <div class="icon-eyelet"><div class="icon-hole"></div></div>
    </div>
    <div class="icon-airmail"></div>
    <div class="icon-head">
      <div class="icon-flight">HS-AIR</div>
      <div class="icon-dest">HND</div>
    </div>
    <div class="icon-checks">
      {#each iconRows as row, index}
        <div class="icon-row">
          <span class="icon-box" data-done={index < marks}>{#if index < marks}✓{/if}</span>
          <span class="icon-line" data-length={row.length}></span>
          <span class="icon-stamp" data-stamp={row.tone}>{row.stamp}</span>
        </div>
      {/each}
    </div>
    <div class="icon-seal">
      <span class="icon-seal-text">{complete ? "PACKED" : "PACKING"}</span>
      <span class="icon-seal-sub">{totalCount > 0 ? `${Math.round(100 * packedCount / totalCount)}% READY` : "EMPTY TAG"}</span>
    </div>
    <div class="icon-barcode-row">
      <div class="icon-barcode"></div>
      <span class="icon-tag-num">#HS-9402</span>
    </div>
  </div>
</div>
