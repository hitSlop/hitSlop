<script lang="ts">
import { onMount } from "svelte";
import { attachments } from "@hitslop/pop/svelte";
import type { YouTubeHandlers } from "@hitslop/pop/embed";
import type { PlaybackController } from "./controller.svelte";

let { id, handlers, onattach }: {
  id: string;
  handlers: YouTubeHandlers;
  onattach: (controller: PlaybackController | null) => void;
} = $props();
let audio: HTMLAudioElement;
let disposed = false;

async function play() {
  try { await audio.play(); }
  catch (error) {
    if (disposed || (error instanceof DOMException && error.name === "AbortError")) return;
    // A phone may require a second tap after a track loads. The visible play button
    // calls this controller directly inside that gesture.
    if (error instanceof DOMException && error.name === "NotAllowedError") handlers.onState?.(2);
    else handlers.onError?.(-2);
  }
}

onMount(() => {
  onattach({
    play: () => { void play(); },
    pause: () => audio.pause(),
    seek: (seconds) => { if (Number.isFinite(audio.duration)) audio.currentTime = Math.min(seconds, audio.duration); },
    // iPhone uses its hardware/system volume. Other browsers accept this value.
    setVolume: (level) => { audio.volume = level; },
  });
  void play();
  return () => { disposed = true; audio.pause(); onattach(null); };
});
</script>

<audio
  bind:this={audio}
  src={attachments.url(id)}
  preload="metadata"
  onloadedmetadata={() => { handlers.onReady?.(); handlers.onTime?.(audio.currentTime, audio.duration); }}
  ontimeupdate={() => handlers.onTime?.(audio.currentTime, audio.duration)}
  onplaying={() => handlers.onState?.(1)}
  onpause={() => handlers.onState?.(2)}
  onwaiting={() => handlers.onState?.(3)}
  onended={() => { handlers.onState?.(0); handlers.onEnded?.(); }}
  onerror={() => handlers.onError?.(-2)}
></audio>
