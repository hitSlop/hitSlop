<script lang="ts">
import Player from "./Player.svelte";
import LocalPlayer from "./LocalPlayer.svelte";
import TrackArt from "./TrackArt.svelte";
import { Button } from "bits-ui";
import TitleBar from "./TitleBar.svelte";
import type { YouTubeHandlers } from "@hitslop/pop/embed";
import type { Playback, PlaybackController } from "./controller.svelte";
import type { Video } from "./pod";

let { video, playback, scrub, volume, volumeVisible, systemVolume, held, handlers, onattach }: {
  video: Video | undefined;
  playback: Playback;
  scrub: boolean;
  volume: number;
  volumeVisible: boolean;
  systemVolume: boolean;
  held: boolean;
  handlers: YouTubeHandlers;
  onattach: (controller: PlaybackController | null) => void;
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
  code === -2 ? "This song couldn’t be played. Try again or import it again."
    : code === -1 ? "Can’t reach the player. Check your connection."
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
    {#if video}<TrackArt class="pod-poster" {video} />{/if}
    {#if playback.started && video}
      {#key video.$id}
        {#if video.audio}
          <LocalPlayer id={video.audio.id} {handlers} {onattach} />
        {:else}
          <Player youtubeId={video.youtubeId} {handlers} {onattach} {waiting} />
        {/if}
      {/key}
    {/if}
    {#if video?.audio && playback.started && playback.ready && playback.state === 2}
      <Button.Root class="pod-local-resume" onclick={() => playback.controller?.play()} aria-label="Play local song">▶ Play</Button.Root>
    {/if}
    {#if !video}
      <p class="pod-empty pod-empty-dark">No songs yet.<br />Add local audio or a YouTube link.</p>
    {/if}
  </div>
  {#if volumeVisible}
    {#if systemVolume}
      <p class="pod-playback-status" role="status">Use the phone’s volume buttons.<br />Press the center to scrub with the wheel.</p>
    {:else}
      <div class="pod-volume" role="img" aria-label={`Volume ${Math.round(volume * 100)} percent`}>
        <svg viewBox="0 0 16 14" aria-hidden="true"><path d="M1 5h3l4-3.5v11L4 9H1zM10.5 4.5a4 4 0 0 1 0 5" /></svg>
        <span class="pod-volume-track"><span style:width="{volume * 100}%"></span></span>
      </div>
    {/if}
  {/if}
  {#if status && video}<p class="pod-playback-status" role="status">{status}</p>{/if}
  {#if video?.audio}
    <div class="pod-track-info"><strong>{video.title}</strong><span>{video.author} · On this Pod</span></div>
  {/if}
  <div class="pod-progress" data-scrub={scrub}>
    <span class="pod-time">{clock(playback.time)}</span>
    <span class="pod-scrubber" role="progressbar" aria-label={scrub ? "Scrub position" : "Position"} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progress * 100)}>
      <span class="pod-scrubber-fill" style:width="{progress * 100}%"></span>
      <i class="pod-scrubber-head" style:left="{progress * 100}%"></i>
    </span>
    <span class="pod-time">{playback.duration ? `-${clock(playback.duration - playback.time)}` : "–:––"}</span>
  </div>
</section>
