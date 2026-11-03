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

let grid = $state<HTMLDivElement>();
let options = $state<HTMLButtonElement[]>([]);
$effect(() => {
  const selected = options[cursor];
  // Follow an active keyboard selection without taking focus from the wheel or page.
  if (selected && grid?.contains(document.activeElement)) selected.focus({ preventScroll: true });
});
</script>

<section class="pod-screen pod-grid" aria-label="Sticker pack">
  <TitleBar title="Sticker Pack" {playing} {paused} {held} />
  <div bind:this={grid} class="pod-grid-body" role="listbox" aria-label="Stickers">
    {#each stickerKinds as kind, index (kind)}
      <button bind:this={options[index]} type="button" role="option" aria-selected={index === cursor} aria-label={kind} data-selected={index === cursor} tabindex={index === cursor ? 0 : -1} onclick={() => onpick(index)}>
        <Sticker {kind} />
      </button>
    {/each}
  </div>
</section>
