<script lang="ts">
  import { onDestroy } from "svelte";
  import Mic from "@lucide/svelte/icons/mic";
  import Square from "@lucide/svelte/icons/square";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import { explain, extensionFor, open, recordingFormat, stop, type Problem } from "./devices";
  import { AUDIO_BITS_PER_SECOND, MAX_RECORDING_SECONDS, clock } from "./notebook";

  let { onrecording }: {
    /** Keeps a finished recording; rejects with a message when it can't. */
    onrecording: (clip: { blob: Blob; ext: string; seconds: number }) => Promise<void>;
  } = $props();

  // The stream, the recorder, the level and the unfinished clip are view state: an
  // unfinished recording is lost if the window closes before Stop and Keep.
  let phase = $state<"idle" | "starting" | "recording" | "saving" | "error">("idle");
  let problem = $state<Problem>();
  let note = $state("");
  let elapsed = $state(0);
  let level = $state(0);
  let stream: MediaStream | undefined;
  let recorder: MediaRecorder | undefined;
  let audio: AudioContext | undefined;
  let frame = 0, ticker: ReturnType<typeof setInterval> | undefined;
  let startedAt = 0, attempt = 0, disposed = false;
  /** How each stopped take ends: kept with its length, or thrown away. */
  const endings = new WeakMap<MediaRecorder, { keep: boolean; seconds: number }>();

  async function record() {
    const format = recordingFormat();
    if (!format) {
      problem = { title: "Recording isn't available", detail: "This window can't record audio. Open the notebook in tinys to record." };
      phase = "error";
      return;
    }
    const mine = ++attempt;
    phase = "starting";
    problem = undefined;
    note = "";
    // Created inside the click, so the level meter may run.
    try { audio = new AudioContext(); } catch { audio = undefined; }
    try {
      const next = await open("microphone");
      if (mine !== attempt || disposed) { stop(next); return; }
      stream = next;
      const chunks: Blob[] = [];
      const take = new MediaRecorder(next, { ...(format.mimeType ? { mimeType: format.mimeType } : {}), audioBitsPerSecond: AUDIO_BITS_PER_SECOND });
      recorder = take;
      take.addEventListener("dataavailable", event => { if (event.data.size) chunks.push(event.data); });
      take.addEventListener("stop", () => {
        const ending = endings.get(take);
        const type = take.mimeType || format.mimeType || chunks[0]?.type || "";
        stop(next);
        if (disposed || !ending?.keep) return;
        void finish(new Blob(chunks, { type }), type, ending.seconds);
      });
      take.addEventListener("error", () => {
        if (recorder !== take) return;
        release();
        problem = { title: "Recording stopped", detail: "The recorder hit a problem, so this recording wasn't kept. Try again." };
        phase = "error";
      });
      for (const track of next.getAudioTracks()) track.addEventListener("ended", () => {
        if (recorder === take && take.state === "recording") {
          note = "The microphone stopped, so the recording was kept up to that point.";
          end(true);
        }
      });
      take.start(1000);
      startedAt = performance.now();
      elapsed = 0;
      phase = "recording";
      ticker = setInterval(() => {
        elapsed = (performance.now() - startedAt) / 1000;
        if (elapsed >= MAX_RECORDING_SECONDS) {
          note = "Recordings stop at 5 minutes. This one was kept.";
          end(true);
        }
      }, 250);
      meter(next);
    } catch (error) {
      release();
      if (mine !== attempt || disposed) return;
      problem = explain(error, "microphone");
      phase = "error";
    }
  }

  function meter(source: MediaStream) {
    if (!audio) return;
    const analyser = audio.createAnalyser();
    analyser.fftSize = 1024;
    audio.createMediaStreamSource(source).connect(analyser);
    const samples = new Float32Array(analyser.fftSize);
    const tick = () => {
      analyser.getFloatTimeDomainData(samples);
      let sum = 0;
      for (const sample of samples) sum += sample * sample;
      const rms = Math.sqrt(sum / samples.length);
      // Roughly -60 dB to 0 dB mapped to 0..1, smoothed so it reads calmly.
      const target = Math.max(0, Math.min(1, (20 * Math.log10(rms + 1e-8) + 60) / 60));
      level = level * 0.7 + target * 0.3;
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
  }

  /** Stops recording; `keep` imports the clip, otherwise it is thrown away. */
  function end(keep: boolean) {
    const take = recorder;
    if (!take || take.state === "inactive") return;
    endings.set(take, { keep, seconds: (performance.now() - startedAt) / 1000 });
    phase = keep ? "saving" : "idle";
    take.stop();
    // The microphone turns off now; the recorder still delivers what it heard.
    release();
  }

  async function finish(blob: Blob, type: string, seconds: number) {
    phase = "saving";
    try {
      if (!blob.size) throw new Error("The microphone sent no sound, so nothing was kept.");
      await onrecording({ blob: new Blob([blob], { type: type.split(";")[0] }), ext: extensionFor(type), seconds });
      phase = "idle";
    } catch (error) {
      problem = { title: "The recording wasn't kept", detail: error instanceof Error ? error.message : "Try recording again." };
      phase = "error";
    }
  }

  /** Turns the microphone off and stops the meter. */
  function release() {
    clearInterval(ticker);
    cancelAnimationFrame(frame);
    stop(stream);
    stream = undefined;
    recorder = undefined;
    level = 0;
    audio?.close().catch(() => undefined);
    audio = undefined;
  }

  function visibility() {
    // A hidden window doesn't keep listening: the recording so far is kept.
    if (document.visibilityState !== "hidden") return;
    if (phase === "recording") {
      note = "The window was hidden, so recording stopped and was kept.";
      end(true);
    } else if (phase === "starting") {
      attempt++;
      release();
      phase = "idle";
    }
  }
  onDestroy(() => { disposed = true; attempt++; recorder?.state === "recording" && recorder.stop(); release(); });

  const bars = 18;
</script>

<svelte:document onvisibilitychange={visibility} />

<section class="cn-tape" data-phase={phase} aria-label="Audio recorder">
  {#if phase === "recording"}
    <div class="cn-tape-live">
      <span class="cn-rec" role="status"><i aria-hidden="true"></i>Recording</span>
      <span class="cn-time"><time>{clock(elapsed)}</time><small>&nbsp;/ {clock(MAX_RECORDING_SECONDS)}</small></span>
      <span class="cn-level" aria-hidden="true">
        {#each { length: bars } as _, index}<i data-on={level * bars > index}></i>{/each}
      </span>
    </div>
    <p class="cn-tape-note">Not saved until you press Stop and Keep.</p>
    <div class="cn-tape-actions">
      <button class="cn-button cn-button-ghost" onclick={() => end(false)}>Discard</button>
      <button class="cn-button cn-button-rec" onclick={() => end(true)}><Square size={14} fill="currentColor" aria-hidden="true" /> Stop and Keep</button>
    </div>
  {:else if phase === "error" && problem}
    <div class="cn-tape-problem" role="alert">
      <TriangleAlert size={20} aria-hidden="true" />
      <div><strong>{problem.title}</strong><p>{problem.detail}</p></div>
    </div>
    <div class="cn-tape-actions">
      <button class="cn-button cn-button-ghost" onclick={() => { phase = "idle"; problem = undefined; }}>Dismiss</button>
      <button class="cn-button cn-button-rec" onclick={record}><Mic size={16} aria-hidden="true" /> Try again</button>
    </div>
  {:else}
    <button class="cn-button cn-button-rec cn-record" disabled={phase !== "idle"} onclick={record}>
      <span class="cn-rec-dot" aria-hidden="true"></span>
      {phase === "starting" ? "Starting microphone…" : phase === "saving" ? "Keeping recording…" : "Record Audio"}
    </button>
    <p class="cn-tape-note" aria-live="polite">{note || (phase === "starting" ? "If macOS asks, allow tinys to use the microphone." : "Up to 5 minutes. Kept when you press Stop and Keep.")}</p>
  {/if}
</section>
