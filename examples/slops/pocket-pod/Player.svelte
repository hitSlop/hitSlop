<script lang="ts">
import { youtube, type YouTubeHandlers, type YouTubePlayer } from "@hitslop/document/embed";

let { youtubeId, handlers, onattach, waiting = false }: {
  youtubeId: string;
  handlers: YouTubeHandlers;
  onattach: (controller: YouTubePlayer | null) => void;
  waiting?: boolean;
} = $props();

let frame = $state<HTMLIFrameElement>();

$effect(() => {
  if (!frame) return;
  const controller = youtube.player(frame, handlers);
  onattach(controller);
  return () => {
    controller.dispose();
    onattach(null);
  };
});
</script>

<iframe
  bind:this={frame}
  class="pod-player"
  title="Video player"
  src={youtube.src(youtubeId, { controls: false })}
  allow="autoplay; encrypted-media"
  tabindex="-1"
  data-waiting={waiting}
></iframe>
