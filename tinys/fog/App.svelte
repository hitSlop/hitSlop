<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { onMount } from "svelte";
  import { attachments, getDocument } from "@hitslop/tinys/svelte";
  import schema from "./schema";
  import { mirrorView, stamp } from "./mirror";
  import Glass from "./Glass.svelte";
  import Mirror from "./Mirror.svelte";

  const doc = getDocument(schema);
  const view = $derived(mirrorView(doc.current));

  // Everything live is view state: the streams and the audio graph here, the trackers in
  // Mirror (they exist only while the devices are on).
  type Session = { video: MediaStream; audio: MediaStream | null; context: AudioContext | null; micProblem: string };
  let phase = $state<"start" | "asking" | "live">("start");
  let session = $state.raw<Session | null>(null);
  let problem = $state<{ title: string; detail: string } | null>(null);
  let notice = $state("");

  function explain(error: unknown, noun: "camera" | "microphone") {
    const name = error instanceof Error ? error.name : "";
    const Noun = noun === "camera" ? "Camera" : "Microphone";
    if (name === "NotAllowedError" || name === "SecurityError")
      return { title: `${Noun} access is off`, detail: `The mirror needs the ${noun}. Allow it for tinys in System Settings › Privacy & Security › ${Noun}, then try again.` };
    if (name === "NotFoundError" || name === "OverconstrainedError")
      return { title: `No ${noun} found`, detail: `Connect a ${noun} or turn it on, then try again.` };
    if (name === "NotReadableError" || name === "AbortError")
      return { title: `The ${noun} is busy`, detail: `Another app may be using the ${noun}. Close it there, then try again.` };
    return { title: `The ${noun} didn't start`, detail: error instanceof Error && error.message ? error.message : "Try again in a moment." };
  }

  function release(current: Session | null) {
    for (const track of current?.video.getTracks() ?? []) track.stop();
    for (const track of current?.audio?.getTracks() ?? []) track.stop();
    void current?.context?.close();
  }

  async function start() {
    if (phase !== "start") return;
    problem = null;
    notice = "";
    if (!navigator.mediaDevices?.getUserMedia) {
      problem = { title: "No camera here", detail: "This window can't use a camera. Open the mirror in tinys." };
      return;
    }
    phase = "asking";
    // Created inside the click so it may start; it only analyses, it never plays.
    const context = typeof AudioContext === "undefined" ? null : new AudioContext();
    let video: MediaStream;
    try {
      video = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "user", width: { ideal: 1280 }, height: { ideal: 720 } }, audio: false });
    } catch (error) {
      void context?.close();
      phase = "start";
      problem = explain(error, "camera");
      return;
    }
    let audio: MediaStream | null = null;
    let micProblem = "";
    try {
      // Breath is a faint hiss: speech filters would remove exactly what we listen for.
      audio = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }, video: false });
      await context?.resume();
    } catch (error) {
      micProblem = `${explain(error, "microphone").title}: hold Space to fog instead.`;
    }
    session = { video, audio, context: audio ? context : null, micProblem };
    if (!audio) void context?.close();
    phase = "live";
  }

  function stop(message = "") {
    release(session);
    session = null;
    phase = "start";
    notice = message;
  }

  onMount(() => {
    const hidden = () => { if (document.hidden && session) stop("Paused while the window was hidden. The camera and microphone are off."); };
    document.addEventListener("visibilitychange", hidden);
    return () => {
      document.removeEventListener("visibilitychange", hidden);
      release(session);
    };
  });
</script>

{#if phase === "live" && session}
  <Mirror
    video={session.video}
    audio={session.audio}
    context={session.context}
    micProblem={session.micProblem}
    onexit={() => stop()}
  />
{:else}
  <main class="fg-start">
    <Glass image={view.snapshot ? attachments.url(view.snapshot.id) : null} alt="The last saved mirror" />
    <section class="fg-start-card" aria-labelledby="fg-title">
      <svg class="fg-start-mark" viewBox="0 0 80 80" fill="none" aria-hidden="true">
        <path d="M23 29v3m34-3v3M23 47q17 21 34 0" />
      </svg>
      <h1 id="fg-title" class="fg-title">fog</h1>
      <p class="fg-subtitle">a little room to exhale</p>
      <div class="fg-start-body">
        {#if problem}
          <div class="fg-problem" role="alert"><strong>{problem.title}</strong><span>{problem.detail}</span></div>
        {:else if notice}
          <p class="fg-notice" role="status">{notice}</p>
        {:else}
          <p class="fg-lede">Breathe to fog. Pinch to draw.<br />Make a fist to wipe.</p>
        {/if}
        <button class="fg-begin" onclick={start} disabled={phase === "asking"}>
          {phase === "asking" ? "Opening your mirror…" : problem ? "Try again" : "Start mirror"}
        </button>
        <p class="fg-private">Your camera images and audio stay on this Mac.<span>The tracker sends anonymous usage metrics to Google, never images or sound.</span></p>
        {#if view.snapshot}<p class="fg-saved">last snapshot · {stamp(view.snapshot.at)}</p>{/if}
      </div>
    </section>
  </main>
{/if}
