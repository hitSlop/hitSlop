<script lang="ts">
  import doc, { CHANNEL_IDS } from "./schema";
</script>

<div class="iconSurface" aria-hidden="true">
  <div class="iconShell">
    <div class="iconTop">
      <span class="iconPill"></span>
      <span class="iconLamp" data-on={doc.current.playing}></span>
    </div>
    <div class="iconDeck">
      {#each CHANNEL_IDS as id}
        <div class="iconSlot">
          <div class="iconCap" style:bottom="{Math.max(8, Math.min(86, doc.current.channels[id]))}%"></div>
        </div>
      {/each}
    </div>
    <div class="iconVu">
      <span></span><span></span><span></span><span></span><span></span><span></span><span data-hot="true"></span>
    </div>
  </div>
</div>
