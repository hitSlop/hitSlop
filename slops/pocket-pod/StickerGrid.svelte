<script lang="ts">
import Sticker from "./Sticker.svelte";
import TitleBar from "./TitleBar.svelte";
import { stickerKinds } from "./schema";

let { cursor, playing, paused, held, onpick }: {
  cursor: number;
  playing?: boolean;
  paused?: boolean;
  held?: boolean;
  onpick: (index: number) => void;
} = $props();
</script>

<section class="pod-screen pod-grid" aria-label="Sticker pack">
  <TitleBar title="Sticker Pack" {playing} {paused} {held} />
  <div class="pod-grid-body" role="listbox" aria-label="Stickers">
    {#each stickerKinds as kind, index (kind)}
      <button type="button" role="option" aria-selected={index === cursor} aria-label={kind} data-selected={index === cursor} tabindex="-1" onclick={() => onpick(index)}>
        <Sticker {kind} />
      </button>
    {/each}
  </div>
</section>
