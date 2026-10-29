<script lang="ts">
  import { onDestroy } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import CameraIcon from "@lucide/svelte/icons/camera";
  import CameraOff from "@lucide/svelte/icons/camera-off";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import { explain, open, still, stop, type Problem } from "./devices";
  import { PHOTO_LONGEST_EDGE } from "./notebook";

  let { onphoto }: {
    /** Keeps a finished photo; rejects with a message when it can't. */
    onphoto: (shot: { blob: Blob; width: number; height: number }) => Promise<void>;
  } = $props();

  // All of this is view state: the stream and the preview never reach the document.
  let phase = $state<"off" | "starting" | "live" | "error">("off");
  let problem = $state<Problem>();
  let note = $state("");
  let saving = $state(false);
  let flash = $state(0);
  let video = $state<HTMLVideoElement>();
  let stream: MediaStream | undefined;
  let attempt = 0;

  async function start() {
    const mine = ++attempt;
    phase = "starting";
    problem = undefined;
    note = "";
    try {
      const next = await open("camera");
      if (mine !== attempt) { stop(next); return; }
      stream = next;
      // A track can end on its own: the camera is unplugged or taken by the system.
      for (const track of next.getVideoTracks()) track.addEventListener("ended", () => {
        if (stream !== next) return;
        turnOff();
        note = "The camera stopped. Turn it on again to keep taking photos.";
      });
      phase = "live";
    } catch (error) {
      if (mine !== attempt) return;
      problem = explain(error, "camera");
      phase = "error";
    }
  }
  function turnOff() {
    attempt++;
    stop(stream);
    stream = undefined;
    if (video) video.srcObject = null;
    phase = "off";
  }
  $effect(() => {
    // Attach the stream once the video element exists in the live state.
    if (phase === "live" && video && stream && video.srcObject !== stream) {
      video.srcObject = stream;
      video.play().catch(() => undefined);
    }
  });

  async function shoot() {
    if (!video || phase !== "live" || saving) return;
    saving = true;
    note = "";
    try {
      const shot = await still(video, PHOTO_LONGEST_EDGE);
      if (!prefersReducedMotion.current) flash++;
      await onphoto(shot);
    } catch (error) {
      note = error instanceof Error ? error.message : "This photo couldn't be kept. Try again.";
    } finally {
      saving = false;
    }
  }

  function visibility() {
    // A hidden window releases the camera; the person turns it back on.
    if (document.visibilityState === "hidden" && (phase === "live" || phase === "starting")) {
      turnOff();
      note = "The camera turned off while the window was hidden.";
    }
  }
  onDestroy(() => { attempt++; stop(stream); stream = undefined; });
</script>

<svelte:document onvisibilitychange={visibility} />

<section class="cn-camera" aria-label="Camera">
  <div class="cn-viewfinder" data-phase={phase}>
    {#if phase === "live"}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video bind:this={video} muted playsinline autoplay aria-label="Camera preview"></video>
      <span class="cn-live" role="status"><i aria-hidden="true"></i>Live · camera on</span>
    {:else}
      <div class="cn-vf-message" role={phase === "error" ? "alert" : "status"}>
        {#if phase === "error" && problem}
          <TriangleAlert size={30} aria-hidden="true" />
          <strong>{problem.title}</strong>
          <p>{problem.detail}</p>
          <button class="cn-button cn-button-light" onclick={start}>Try again</button>
        {:else if phase === "starting"}
          <CameraIcon size={30} aria-hidden="true" />
          <strong>Starting the camera…</strong>
          <p>If macOS asks, allow tinys to use the camera.</p>
        {:else}
          <CameraOff size={30} aria-hidden="true" />
          <strong>Camera is off</strong>
          <p>Nothing is seen or saved until you take a photo.</p>
          <button class="cn-button cn-button-light" onclick={start}>Turn on camera</button>
        {/if}
      </div>
    {/if}
    <span class="cn-corner" data-corner="tl" aria-hidden="true"></span>
    <span class="cn-corner" data-corner="tr" aria-hidden="true"></span>
    <span class="cn-corner" data-corner="bl" aria-hidden="true"></span>
    <span class="cn-corner" data-corner="br" aria-hidden="true"></span>
    {#key flash}{#if flash}<span class="cn-flash" aria-hidden="true"></span>{/if}{/key}
  </div>
  <div class="cn-deck">
    <button class="cn-switch" data-on={phase === "live"} disabled={phase === "starting"}
      aria-label={phase === "live" ? "Turn camera off" : "Turn camera on"}
      onclick={() => (phase === "live" ? turnOff() : start())}>
      {#if phase === "live"}<CameraOff size={17} aria-hidden="true" /> Turn off{:else}<CameraIcon size={17} aria-hidden="true" /> Camera{/if}
    </button>
    <button class="cn-shutter" disabled={phase !== "live" || saving} onclick={shoot}>
      <span class="cn-shutter-disc" aria-hidden="true"></span>
      {saving ? "Keeping…" : "Take Photo"}
    </button>
    <p class="cn-deck-note" aria-live="polite">{note || (phase === "live" ? "Each photo is kept the moment you take it." : "Photos are kept in this document.")}</p>
  </div>
</section>
