<script lang="ts">
import Player from "./Player.svelte";
import TitleBar from "./TitleBar.svelte";
import { youtube, type YouTubeHandlers, type YouTubePlayer } from "@hitslop/document/embed";
import type { Playback } from "./pod.svelte";
import type { Video } from "./schema";

let { video, playback, scrub, volume, volumeVisible, held, handlers, onattach }: {
  video: Video | undefined;
  playback: Playback;
  scrub: boolean;
  volume: number;
  volumeVisible: boolean;
  held: boolean;
  handlers: YouTubeHandlers;
  onattach: (controller: YouTubePlayer | null) => void;
} = $props();

const clock = (seconds: number) => {
  if (!Number.isFinite(seconds) || seconds < 0) return "–:––";
  const whole = Math.floor(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = String(whole % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
};
// If the browser won't autoplay, let a click reach the player's own start button.
const waiting = $derived(playback.ready && playback.error === null && (playback.state === -1 || playback.state === 5));
const progress = $derived(playback.duration ? Math.min(1, playback.time / playback.duration) : 0);
const failure = (code: number) =>
  code === -1 ? "Can’t reach the player. Check your connection."
    : code === 100 ? "That video is no longer available."
    : code === 101 || code === 150 ? "The owner doesn’t allow this video to play here."
    : "This video won’t play here. Try the next one.";
const status = $derived.by(() => {
  if (!playback.started) return "Press ▶⏸ to play";
  if (playback.error !== null) return failure(playback.error);
  if (!playback.ready) return "Loading…";
  if (playback.state === 3) return "Buffering…";
  if (playback.state === 1) return "";
  if (playback.state === 2) return "Paused";
  return "Press ▶⏸ to play";
});
</script>

<section class="pod-screen pod-playing" aria-label="Now Playing">
  <TitleBar
    title={video?.title ?? "Now Playing"}
    playing={playback.started && playback.state === 1}
    paused={playback.started && playback.state === 2}
    {held}
  />
  <div class="pod-video">
    {#if video}<img class="pod-poster" src={youtube.thumbnail(video.youtubeId)} alt="" draggable="false" />{/if}
    {#if playback.started && video}
      {#key video.youtubeId}
        <Player youtubeId={video.youtubeId} {handlers} {onattach} {waiting} />
      {/key}
    {/if}
    {#if !video}
      <p class="pod-empty pod-empty-dark">No videos yet.<br />Paste a YouTube link to add one.</p>
    {:else if status && !volumeVisible}
      <p class="pod-video-status" role="status">{status}</p>
    {/if}
    {#if volumeVisible}
      <div class="pod-volume" role="img" aria-label={`Volume ${Math.round(volume * 100)} percent`}>
        <svg viewBox="0 0 16 14" aria-hidden="true"><path d="M1 5h3l4-3.5v11L4 9H1zM10.5 4.5a4 4 0 0 1 0 5" /></svg>
        <span class="pod-volume-track"><span style:width="{volume * 100}%"></span></span>
      </div>
    {/if}
  </div>
  <div class="pod-progress" data-scrub={scrub}>
    <span class="pod-time">{clock(playback.time)}</span>
    <span class="pod-scrubber" role="progressbar" aria-label={scrub ? "Scrub position" : "Position"} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progress * 100)}>
      <span class="pod-scrubber-fill" style:width="{progress * 100}%"></span>
      <i class="pod-scrubber-head" style:left="{progress * 100}%"></i>
    </span>
    <span class="pod-time">{playback.duration ? `-${clock(playback.duration - playback.time)}` : "–:––"}</span>
  </div>
</section>
