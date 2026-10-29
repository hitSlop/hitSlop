<script lang="ts">
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import { clock } from "./notebook";

  // Playback position is view state. One clip plays at a time across the notebook.
  let { src, seconds, label }: { src: string; seconds: number; label: string } = $props();
  let player = $state<HTMLAudioElement>();
  let playing = $state(false);
  let position = $state(0);
  let failed = $state(false);
  const progress = $derived(seconds > 0 ? Math.min(1, position / seconds) : 0);

  function toggle() {
    if (!player) return;
    if (playing) { player.pause(); return; }
    document.dispatchEvent(new CustomEvent("cn-play", { detail: player }));
    player.play().catch(() => (failed = true));
  }
  function seek(event: MouseEvent) {
    if (!player || !seconds) return;
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    player.currentTime = Math.max(0, Math.min(seconds, ((event.clientX - box.left) / box.width) * seconds));
  }
  function keyseek(event: KeyboardEvent) {
    if (!player) return;
    const step = event.key === "ArrowRight" ? 5 : event.key === "ArrowLeft" ? -5 : 0;
    if (!step) return;
    event.preventDefault();
    player.currentTime = Math.max(0, Math.min(seconds || player.duration || 0, player.currentTime + step));
  }
  $effect(() => {
    const other = (event: Event) => { if ((event as CustomEvent).detail !== player) player?.pause(); };
    document.addEventListener("cn-play", other);
    return () => { document.removeEventListener("cn-play", other); player?.pause(); };
  });
</script>

<span class="cn-player" data-playing={playing}>
  <button class="cn-play" onclick={toggle} disabled={failed} aria-label={`${playing ? "Pause" : "Play"} ${label}`}>
    {#if playing}<Pause size={16} fill="currentColor" aria-hidden="true" />{:else}<Play size={16} fill="currentColor" aria-hidden="true" />{/if}
  </button>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <span class="cn-track" role="slider" tabindex="0" aria-label={`Position in ${label}`}
    aria-valuemin={0} aria-valuemax={Math.round(seconds)} aria-valuenow={Math.round(position)} aria-valuetext={`${clock(position)} of ${clock(seconds)}`}
    onclick={seek} onkeydown={keyseek}>
    <i style:transform={`scaleX(${progress})`}></i>
  </span>
  <span class="cn-player-time">{failed ? "Can't play" : playing || position ? `${clock(position)} / ${clock(seconds)}` : clock(seconds)}</span>
  <audio bind:this={player} {src} preload="metadata"
    onplay={() => (playing = true)} onpause={() => (playing = false)}
    onended={() => { playing = false; position = 0; }}
    ontimeupdate={() => (position = player?.currentTime ?? 0)}
    onerror={() => (failed = true)}></audio>
</span>
