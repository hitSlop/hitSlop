<script lang="ts">
import type { Snippet } from "svelte";

// The 360×603 body. Stickers sit under the screen and the wheel, so they can never block a control.
let { stickers, screen, tray, wheel, hold, dock }: {
  stickers: Snippet;
  screen: Snippet;
  tray: Snippet;
  wheel: Snippet;
  hold?: Snippet;
  dock?: Snippet;
} = $props();
</script>

<div class="pod">
  {@render stickers()}
  <div class="pod-bezel"><div class="pod-glass">{@render screen()}</div></div>
  <div class="pod-tray">{@render tray()}</div>
  <div class="pod-wheel-slot">{@render wheel()}</div>
  {#if dock}<div class="pod-dock">{@render dock()}</div>{/if}
  {#if hold}{@render hold()}{/if}
</div>
