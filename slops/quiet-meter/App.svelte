<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { onMount } from "svelte";
  import { EditableText, notify } from "hitslop/svelte";
  import Mic from "@lucide/svelte/icons/mic";
  import Square from "@lucide/svelte/icons/square";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import doc from "./schema";
  import { LEVEL, SUSTAIN } from "./schema";
  import {
    ZONE_LABEL, advanceTally, clearSessions, clock, emptyTally, levelFromRms, localDay, meterView,
    saveSession, setSustain, setThresholds, smoothLevel, zoneOf, dayLabel, type Tally,
  } from "./meter";
  import Gauge from "./Gauge.svelte";
  import Lamp from "./Lamp.svelte";
  import Summary from "./Summary.svelte";

  const { last, lastZone, recent } = $derived(meterView(doc.current));
  const amber = $derived(doc.current.amber);
  const red = $derived(doc.current.red);
  const sustain = $derived(doc.current.sustain);

  // View state: the microphone, the smoothed level and the running tally are transient and never stored.
  let phase = $state<"idle" | "starting" | "live" | "blocked">("idle");
  let problem = $state("");
  let level = $state(0);
  let shown = $state(0);
  let tally = $state<Tally>(emptyTally());
  let dragAmber = $state<number | null>(null);
  let dragRed = $state<number | null>(null);

  const liveAmber = $derived(dragAmber ?? amber);
  const liveRed = $derived(dragRed ?? red);
  const zone = $derived(zoneOf(level, liveAmber, liveRed));
  const sustained = $derived(phase === "live" && tally.run >= sustain);

  let stream: MediaStream | undefined;
  let context: AudioContext | undefined;
  let frame = 0;
  let shownAt = 0;

  function explain(error: unknown) {
    const name = error instanceof DOMException ? error.name : "";
    if (name === "NotAllowedError" || name === "SecurityError") return "Microphone access is off. Allow it for hitSlop in System Settings > Privacy & Security > Microphone, then press Listen again.";
    if (name === "NotFoundError" || name === "OverconstrainedError") return "No microphone was found. Connect one and press Listen again.";
    if (name === "NotReadableError" || name === "AbortError") return "The microphone is busy in another app. Close it there and press Listen again.";
    return "The microphone could not be started.";
  }

  async function start() {
    if (phase === "starting" || phase === "live") return;
    phase = "starting";
    problem = "";
    try {
      // Raw room level: speech filters would flatten exactly what we want to measure.
      stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false },
      });
      context = new AudioContext();
      await context.resume();
      const analyser = context.createAnalyser();
      analyser.fftSize = 2048;
      context.createMediaStreamSource(stream).connect(analyser);
      const samples = new Float32Array(analyser.fftSize);
      level = 0; shown = 0; tally = emptyTally();
      phase = "live";
      let before = performance.now();
      shownAt = before;
      const tick = (now: number) => {
        const dt = Math.min(0.25, (now - before) / 1000);
        before = now;
        analyser.getFloatTimeDomainData(samples);
        let sum = 0;
        for (const v of samples) sum += v * v;
        level = smoothLevel(level, levelFromRms(Math.sqrt(sum / samples.length)), dt);
        tally = advanceTally(tally, level, zoneOf(level, doc.current.amber, doc.current.red), dt, doc.current.sustain);
        if (now - shownAt > 120) { shown = level; shownAt = now; }
        frame = requestAnimationFrame(tick);
      };
      frame = requestAnimationFrame(tick);
    } catch (error) {
      release();
      phase = "blocked";
      problem = explain(error);
    }
  }

  function release() {
    cancelAnimationFrame(frame);
    stream?.getTracks().forEach(track => track.stop());
    stream = undefined;
    void context?.close();
    context = undefined;
  }

  async function stop(reason: "stopped" | "hidden" = "stopped") {
    if (phase !== "live") return;
    release();
    const result = tally;
    phase = "idle";
    level = 0; shown = 0; tally = emptyTally();
    const seconds = Math.floor(result.seconds);
    if (seconds < 3) { notify("Too short to save a summary."); return; }
    const loudSeconds = Math.min(seconds, Math.round(result.loudSeconds));
    await doc.change(tx => saveSession(tx, doc.current, {
      date: localDay(new Date()), seconds, loudSeconds, episodes: result.episodes, peak: Math.round(result.peak),
    }));
    notify(reason === "hidden" ? "Window hidden: microphone off, summary saved." : "Microphone off. Summary saved.");
  }

  onMount(() => {
    const hide = () => { if (document.hidden) void stop("hidden"); };
    document.addEventListener("visibilitychange", hide);
    return () => { document.removeEventListener("visibilitychange", hide); release(); };
  });

  const commitThresholds = (nextAmber: number, nextRed: number) => doc.change(tx => setThresholds(tx, nextAmber, nextRed));
  const commitAmber = (event: Event) => {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    dragAmber = null;
    return commitThresholds(value, red);
  };
  const commitRed = (event: Event) => {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    dragRed = null;
    return commitThresholds(amber, value);
  };
  const nudgeSustain = (by: number) => doc.change(tx => setSustain(tx, Math.min(SUSTAIN.max, Math.max(SUSTAIN.min, sustain + by))));
  async function clearAll() {
    const count = await doc.change(tx => clearSessions(tx, doc.current));
    notify(`${count} ${count === 1 ? "summary" : "summaries"} cleared.`);
  }

  const lamp = $derived(phase === "live" ? zone : "off");
</script>

<main class="qm-shell">
  <header class="qm-head">
    <p class="qm-eyebrow">Quiet Meter</p>
    <EditableText class="qm-room" field={doc.fields.room} label="Room name" placeholder="Name the room" />
  </header>

  <section class="qm-housing" aria-label="Live level">
    <Lamp zone={lamp} value={phase === "live" ? shown : null} caption={phase === "live" ? "level, 0 to 100" : "microphone off"} />
    <p class="qm-status" data-zone={lamp} role="status">
      {#if phase === "live"}{ZONE_LABEL[zone]}{sustained ? ` for ${sustain} s+` : ""}{:else if phase === "starting"}Starting…{:else}Not listening{/if}
    </p>
    <Gauge value={phase === "live" ? level : null} amber={liveAmber} red={liveRed} />
    {#if phase === "live"}
      <button class="qm-listen" data-live="true" onclick={() => stop()}>
        <Square size={18} aria-hidden="true" /> Stop and save summary
      </button>
      <p class="qm-live"><span class="qm-dot"></span> Listening · level only, nothing is recorded</p>
    {:else}
      <button class="qm-listen" onclick={start} disabled={phase === "starting"}>
        <Mic size={18} aria-hidden="true" /> Listen
      </button>
      {#if !problem}<p class="qm-live qm-live-off">Microphone is off. Press Listen to measure this room.</p>{/if}
    {/if}
    {#if problem}<p class="qm-problem" role="alert">{problem}</p>{/if}
    <dl class="qm-tally" aria-label="This session (not saved until you stop)">
      <div><dt>Listening</dt><dd>{clock(tally.seconds)}</dd></div>
      <div><dt>Too loud</dt><dd>{clock(tally.loudSeconds)}</dd></div>
      <div><dt>Episodes</dt><dd>{tally.episodes}</dd></div>
    </dl>
  </section>

  <section class="qm-panel" aria-label="Thresholds">
    <h2>Thresholds</h2>
    <label class="qm-slider" data-zone="warn">
      <span>Amber from <b>{liveAmber}</b></span>
      <input type="range" min={LEVEL.min} max={LEVEL.max} step="1" value={liveAmber}
        oninput={(e) => { const v = Math.min(Number(e.currentTarget.value), liveRed - 5); e.currentTarget.value = String(v); dragAmber = v; }} onchange={commitAmber} />
    </label>
    <label class="qm-slider" data-zone="loud">
      <span>Red from <b>{liveRed}</b></span>
      <input type="range" min={LEVEL.min} max={LEVEL.max} step="1" value={liveRed}
        oninput={(e) => { const v = Math.max(Number(e.currentTarget.value), liveAmber + 5); e.currentTarget.value = String(v); dragRed = v; }} onchange={commitRed} />
    </label>
    <div class="qm-sustain">
      <span id="qm-sustain-label">Count as too loud after red for</span>
      <div class="qm-stepper" role="group" aria-labelledby="qm-sustain-label">
        <button aria-label="Fewer seconds" onclick={() => nudgeSustain(-1)} disabled={sustain <= SUSTAIN.min}><Minus size={16} /></button>
        <output>{sustain} s</output>
        <button aria-label="More seconds" onclick={() => nudgeSustain(1)} disabled={sustain >= SUSTAIN.max}><Plus size={16} /></button>
      </div>
    </div>
    <p class="qm-note">Levels are relative to this microphone, not calibrated decibels.</p>

    <div class="qm-section-head">
      <h2>Last session</h2>
      {#if doc.current.sessions.length}
        <button class="qm-quiet-button" onclick={clearAll}><Trash2 size={14} aria-hidden="true" /> Clear</button>
      {/if}
    </div>
    <Summary session={last} zone={lastZone} />
    {#if recent.length > 1}
      <ol class="qm-history">
        {#each recent.slice(1) as row (row.$id)}
          <li><span>{dayLabel(row.date)}</span><span>{row.episodes} {row.episodes === 1 ? "episode" : "episodes"}</span><span>peak {row.peak}</span></li>
        {/each}
      </ol>
    {/if}
  </section>
</main>
