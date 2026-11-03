<script lang="ts">
import TitleBar from "./TitleBar.svelte";
import TrackArt from "./TrackArt.svelte";
import type { Video } from "./pod";

let { video, playing, paused, held }: { video: Video | undefined; playing?: boolean; paused?: boolean; held?: boolean } = $props();
</script>

<section class="pod-screen pod-remove" aria-label="Remove Track">
  <TitleBar title="Remove Track" {playing} {paused} {held} />
  <div class="pod-remove-body">
    {#if video}
      <TrackArt {video} />
      <p class="pod-remove-title">Remove “{video.title}”?</p>
      <p class="pod-add-hint">Press ● to remove it. Press MENU to keep it.</p>
    {:else}
      <p class="pod-add-hint">That track is already gone. Press MENU to go back.</p>
    {/if}
  </div>
</section>
