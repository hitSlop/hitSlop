<script lang="ts">
import TitleBar from "./TitleBar.svelte";
import { youtube } from "@hitslop/document/embed";
import type { Video } from "./schema";

let { video, playing, paused, held }: { video: Video | undefined; playing?: boolean; paused?: boolean; held?: boolean } = $props();
</script>

<section class="pod-screen pod-remove" aria-label="Remove Video">
  <TitleBar title="Remove Video" {playing} {paused} {held} />
  <div class="pod-remove-body">
    {#if video}
      <img src={youtube.thumbnail(video.youtubeId)} alt="" draggable="false" />
      <p class="pod-remove-title">Remove “{video.title}”?</p>
      <p class="pod-add-hint">Press ● to remove it. Press MENU to keep it.</p>
    {:else}
      <p class="pod-add-hint">That video is already gone. Press MENU to go back.</p>
    {/if}
  </div>
</section>
