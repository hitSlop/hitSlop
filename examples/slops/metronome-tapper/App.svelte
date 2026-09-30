<script lang="ts">
  import { Slop, bindValue, useDocument } from "@hitslop/document/svelte";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { RadioGroup, Toggle } from "bits-ui";
  import { onDestroy, onMount } from "svelte";
  import { Spring, prefersReducedMotion } from "svelte/motion";
  import schema, { signatures, type TimeSignature } from "./schema";

  const CAPTURE_ANGLE = -16;
  const SWING_ANGLE = 22;

  const doc = useDocument(schema);

  let isPlaying = $state(false);
  let currentBeat = $state(0);
  let tapTimes: number[] = [];
  let lastTapTime = 0;
  let tapFeedback = $state(false);
  let tapCount = $state(0);
  let audioCtx: AudioContext | null = null;
  let timerId: number | null = null;
  let nextNoteTime = 0;
  let nextBeatNumber = 0;
  let noiseBuffer: AudioBuffer | null = null;
  const pendulum = new Spring(0, { stiffness: 0.18, damping: 0.52 });

  const beatsPerBar = $derived(Number.parseInt(doc.current.signature.split("/")[0] ?? "4", 10));
  const tempoName = $derived.by(() => {
    const bpm = doc.current.bpm;
    if (bpm < 60) return "Largo";
    if (bpm < 76) return "Adagio";
    if (bpm < 108) return "Andante";
    if (bpm < 120) return "Moderato";
    if (bpm < 156) return "Allegro";
    if (bpm < 200) return "Vivace";
    return "Presto";
  });
  const weight = $derived(`${((doc.current.bpm - 40) / 200) * 58 + 18}%`);

  function isSignature(value: string): value is TimeSignature {
    return (signatures as readonly string[]).includes(value);
  }

  function getAudioContext(): AudioContext {
    if (!audioCtx) {
      const AudioCtxClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      audioCtx = new AudioCtxClass();
    }
    if (audioCtx.state === "suspended") void audioCtx.resume();
    return audioCtx;
  }

  function getNoiseBuffer(ctx: AudioContext): AudioBuffer {
    if (noiseBuffer && noiseBuffer.sampleRate === ctx.sampleRate) return noiseBuffer;
    const length = Math.floor(ctx.sampleRate * 0.08);
    const buffer = ctx.createBuffer(1, length, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < length; i += 1) data[i] = Math.random() * 2 - 1;
    noiseBuffer = buffer;
    return buffer;
  }

  function playTick(time: number, isAccent: boolean): void {
    if (doc.current.muted) return;
    try {
      const ctx = getAudioContext();
      const vol = Math.max(0.02, doc.current.volume);
      const noise = ctx.createBufferSource();
      noise.buffer = getNoiseBuffer(ctx);
      const filter = ctx.createBiquadFilter();
      filter.type = "bandpass";
      filter.frequency.setValueAtTime(isAccent ? 1750 : 920, time);
      filter.Q.setValueAtTime(isAccent ? 3.2 : 7.5, time);
      const gain = ctx.createGain();
      gain.gain.setValueAtTime(vol * (isAccent ? 0.9 : 0.55), time);
      gain.gain.exponentialRampToValueAtTime(0.001, time + (isAccent ? 0.045 : 0.028));
      noise.connect(filter);
      filter.connect(gain);
      gain.connect(ctx.destination);
      noise.start(time);
      noise.stop(time + 0.06);

      if (isAccent) {
        const osc = ctx.createOscillator();
        const oscGain = ctx.createGain();
        osc.type = "triangle";
        osc.frequency.setValueAtTime(2100, time);
        osc.frequency.exponentialRampToValueAtTime(780, time + 0.03);
        oscGain.gain.setValueAtTime(vol * 0.35, time);
        oscGain.gain.exponentialRampToValueAtTime(0.001, time + 0.04);
        osc.connect(oscGain);
        oscGain.connect(ctx.destination);
        osc.start(time);
        osc.stop(time + 0.045);
      }
    } catch {
      // Audio context may be blocked until a gesture.
    }
  }

  function scheduler(): void {
    const ctx = getAudioContext();
    while (nextNoteTime < ctx.currentTime + 0.1) {
      const beat = nextBeatNumber;
      const accent = doc.current.signature === "6/8" ? beat === 0 || beat === 3 : beat === 0;
      playTick(nextNoteTime, accent);
      const delayMs = Math.max(0, (nextNoteTime - ctx.currentTime) * 1000);
      const capturedBeat = beat;
      setTimeout(() => {
        if (!isPlaying) return;
        currentBeat = capturedBeat;
      }, delayMs);
      const secondsPerBeat = doc.current.signature === "6/8"
        ? 60 / doc.current.bpm / 2
        : 60 / doc.current.bpm;
      nextNoteTime += secondsPerBeat;
      nextBeatNumber = (nextBeatNumber + 1) % beatsPerBar;
    }
  }

  function start(): void {
    const ctx = getAudioContext();
    isPlaying = true;
    currentBeat = 0;
    nextBeatNumber = 0;
    nextNoteTime = ctx.currentTime + 0.05;
    timerId = window.setInterval(scheduler, 25);
  }

  function stop(): void {
    isPlaying = false;
    if (timerId !== null) {
      clearInterval(timerId);
      timerId = null;
    }
    currentBeat = 0;
  }

  function togglePlay(): void {
    if (isPlaying) stop();
    else start();
  }

  function setBpm(value: number): void {
    const bpm = Math.min(240, Math.max(40, Math.round(value)));
    if (bpm === doc.current.bpm) return;
    doc.fields.bpm.set(bpm);
  }

  function nudgeBpm(delta: number): void {
    setBpm(doc.current.bpm + delta);
  }

  function handleTap(): void {
    const nowTime = performance.now();
    tapFeedback = true;
    window.setTimeout(() => { tapFeedback = false; }, 100);
    if (nowTime - lastTapTime > 2500) {
      tapTimes = [];
      tapCount = 0;
    }
    lastTapTime = nowTime;
    tapTimes.push(nowTime);
    tapCount = tapTimes.length;
    if (tapTimes.length > 6) tapTimes.shift();
    if (tapTimes.length >= 2) {
      let totalDelta = 0;
      for (let i = 1; i < tapTimes.length; i += 1) totalDelta += tapTimes[i]! - tapTimes[i - 1]!;
      setBpm(60000 / (totalDelta / (tapTimes.length - 1)));
    }
  }

  function savePreset(): void {
    const bpm = doc.current.bpm;
    const current = doc.current.presets;
    if (current.includes(bpm)) return;
    const merged = [...current, bpm].sort((a, b) => a - b).slice(0, 6);
    if (merged.length === current.length && merged.every((value, index) => value === current[index])) return;
    doc.change((tx) => {
      const count = Math.min(current.length, merged.length);
      for (let index = 0; index < count; index += 1) {
        if (current[index] !== merged[index]) tx.fields.presets.set(index, merged[index]!);
      }
      if (merged.length > current.length) {
        for (let index = current.length; index < merged.length; index += 1) tx.fields.presets.insert(merged[index]!, index);
      } else if (current.length > merged.length) {
        tx.fields.presets.remove(merged.length, current.length - merged.length);
      }
    });
  }

  function removePreset(bpm: number): void {
    const index = doc.current.presets.indexOf(bpm);
    if (index < 0 || doc.current.presets.length <= 1) return;
    doc.fields.presets.remove(index);
  }

  function handleKeyDown(event: KeyboardEvent): void {
    if (event.target instanceof HTMLInputElement) return;
    if (event.code === "Space") {
      event.preventDefault();
      togglePlay();
    } else if (event.code === "KeyT") {
      event.preventDefault();
      handleTap();
    } else if (event.code === "ArrowUp") {
      event.preventDefault();
      nudgeBpm(event.shiftKey ? 5 : 1);
    } else if (event.code === "ArrowDown") {
      event.preventDefault();
      nudgeBpm(event.shiftKey ? -5 : -1);
    }
  }

  $effect(() => {
    const playing = isPlaying;
    const reduced = prefersReducedMotion.current;
    if (reduced || !playing) {
      void pendulum.set(0, { instant: true });
      return;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let dir = 1;
    const beat = () => {
      if (cancelled) return;
      dir *= -1;
      const bpm = Math.max(40, doc.current.bpm);
      pendulum.stiffness = Math.min(0.42, Math.max(0.1, bpm / 420));
      pendulum.damping = 0.52;
      void pendulum.set(dir * SWING_ANGLE);
      timer = setTimeout(beat, (60 / bpm) * 1000);
    };
    beat();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
  });

  onDestroy(() => {
    window.removeEventListener("keydown", handleKeyDown);
    stop();
    void pendulum.set(pendulum.target, { instant: true });
    if (audioCtx) void audioCtx.close();
  });
</script>

<Slop>
<main class="metronome-shell" data-slop-selection="none" aria-label="Metronome">
  <header class="chassis-head">
    <span class="screw" aria-hidden="true"></span>
    <p>PRECISION TEMPO</p>
    <span class="screw" aria-hidden="true"></span>
  </header>

  <section class="display-card">
    <div class="display-readout">
      <strong class="bpm-digits">{doc.current.bpm}</strong>
      <span class="bpm-unit">BPM</span>
    </div>
    <p class="tempo-descriptor">{tempoName}</p>
    <div class="beat-lights" aria-label={`Beat ${currentBeat + 1} of ${beatsPerBar}`}>
      {#each Array(beatsPerBar) as _, index}
        <span class="beat-dot" class:active={isPlaying && currentBeat === index} class:accent={index === 0}></span>
      {/each}
    </div>
  </section>

  <section class="pendulum-chamber" aria-hidden="true">
    <div class="scale-grooves">
      <span>200</span>
      <span>160</span>
      <span>120</span>
      <span>90</span>
      <span>60</span>
    </div>
    <div
      class="pendulum-arm"
      style:transform="rotate({pendulum.current}deg)"
      style:--weight={weight}
    >
      <div class="brass-rod"></div>
      <div class="brass-weight"><i></i></div>
      <div class="pendulum-pivot"></div>
    </div>
  </section>

  <section class="controls-panel">
    <div class="preset-row" data-slop-export="hide">
      {#each doc.current.presets as preset, index (index)}
        <button
          type="button"
          class="preset-btn"
          class:selected={doc.current.bpm === preset}
          onclick={() => setBpm(preset)}
          ondblclick={() => removePreset(preset)}
        >
          {preset}
        </button>
      {/each}
      <button type="button" class="preset-save" onclick={savePreset} aria-label="Save current tempo">Save</button>
    </div>

    <div class="tempo-nudge-row" data-slop-export="hide">
      <button type="button" class="nudge-btn" onclick={() => nudgeBpm(-5)} aria-label="Subtract 5 BPM">-5</button>
      <button type="button" class="nudge-btn" onclick={() => nudgeBpm(-1)} aria-label="Subtract 1 BPM">-1</button>
      <input
        class="bpm-slider"
        type="range"
        min="40"
        max="240"
        step="1"
        aria-label="Tempo"
        style:--bpm={doc.current.bpm}
        use:bindValue={doc.fields.bpm}
      />
      <button type="button" class="nudge-btn" onclick={() => nudgeBpm(1)} aria-label="Add 1 BPM">+1</button>
      <button type="button" class="nudge-btn" onclick={() => nudgeBpm(5)} aria-label="Add 5 BPM">+5</button>
    </div>

    <div class="settings-row">
      <RadioGroup.Root
        value={doc.current.signature}
        onValueChange={(value) => { if (value && isSignature(value)) { doc.fields.signature.set(value); currentBeat = 0; nextBeatNumber = 0; } }}
        class="time-sig-selector"
        aria-label="Time signature"
      >
        {#each signatures as signature}
          <RadioGroup.Item value={signature} class="sig-btn">{signature}</RadioGroup.Item>
        {/each}
      </RadioGroup.Root>
      <label class="volume-field" data-slop-export="hide">
        {#if doc.current.muted}
          <VolumeX size={15} />
        {:else}
          <Volume2 size={15} />
        {/if}
        <input
          class="volume-slider"
          type="range"
          min="0"
          max="1"
          step="0.01"
          aria-label="Click volume"
          style:--volume={doc.current.volume}
          use:bindValue={doc.fields.volume}
        />
      </label>
      <Toggle.Root
        pressed={doc.current.muted}
        onPressedChange={(pressed) => { doc.fields.muted.set(pressed); }}
        class="mute-toggle"
        aria-label={doc.current.muted ? "Unmute click" : "Mute click"}
      >
        {doc.current.muted ? "MUTE" : "CLICK"}
      </Toggle.Root>
    </div>

    <div class="primary-actions" data-slop-export="hide">
      <button
        type="button"
        class="tap-tempo-btn"
        class:tap-active={tapFeedback}
        onclick={handleTap}
        aria-label="Tap tempo four times"
      >
        <span>TAP TEMPO</span>
        <strong>TAP 4×</strong>
        <em>{tapCount > 0 ? `${Math.min(tapCount, 4)} / 4` : "listen"}</em>
      </button>
      <button
        type="button"
        class="play-toggle-btn"
        class:playing={isPlaying}
        onclick={togglePlay}
        aria-label={isPlaying ? "Stop metronome" : "Start metronome"}
      >
        {#if isPlaying}
          <Square size={22} fill="currentColor" />
        {:else}
          <Play size={24} fill="currentColor" class="play-icon-offset" />
        {/if}
      </button>
    </div>
  </section>
</main>

{#snippet exportView()}
  <main class="metronome-shell" aria-label="Exported metronome">
    <header class="chassis-head">
      <span class="screw" aria-hidden="true"></span>
      <p>PRECISION TEMPO</p>
      <span class="screw" aria-hidden="true"></span>
    </header>

    <section class="display-card">
      <div class="display-readout">
        <strong class="bpm-digits">{doc.current.bpm}</strong>
        <span class="bpm-unit">BPM</span>
      </div>
      <p class="tempo-descriptor">{tempoName}</p>
      <div class="beat-lights" aria-label={`${beatsPerBar} beats per bar`}>
        {#each Array(beatsPerBar) as _, index}
          <span class="beat-dot" class:accent={index === 0}></span>
        {/each}
      </div>
    </section>

    <section class="pendulum-chamber" aria-hidden="true">
      <div class="scale-grooves">
        <span>200</span>
        <span>160</span>
        <span>120</span>
        <span>90</span>
        <span>60</span>
      </div>
      <div class="pendulum-arm" style:transform="rotate({CAPTURE_ANGLE}deg)" style:--weight={weight}>
        <div class="brass-rod"></div>
        <div class="brass-weight"><i></i></div>
        <div class="pendulum-pivot"></div>
      </div>
    </section>

    <section class="controls-panel">
      <div class="settings-row">
        <div class="time-sig-selector" aria-label="Time signature">
          {#each signatures as value}
            <span class="sig-btn" data-state={doc.current.signature === value ? "checked" : undefined}>{value}</span>
          {/each}
        </div>
        <span class="mute-toggle" data-state={doc.current.muted ? "on" : undefined}>{doc.current.muted ? "MUTE" : "CLICK"}</span>
      </div>
    </section>
  </main>
{/snippet}

{#snippet icon()}
  <div style="width:512px;height:512px;display:grid;place-items:center" aria-hidden="true">
    <article class="metronome-icon">
      <div class="icon-top-plate">
        <span class="icon-brand-dot"></span>
        <span class="icon-brand-text">TEMPO INSTRUMENT</span>
      </div>
      <div class="icon-face">
        <div class="icon-bpm-card">
          <span class="icon-bpm-num">{doc.current.bpm}</span>
          <span class="icon-bpm-lbl">{tempoName}</span>
        </div>
        <div class="icon-pendulum-chamber">
          <div class="icon-scale-lines">
            <i></i><i></i><i></i><i></i><i></i><i></i><i></i>
          </div>
          <div class="icon-rod">
            <div class="icon-weight"></div>
          </div>
        </div>
      </div>
      <div class="icon-controls">
        <span class="icon-tap-btn">TAP</span>
        <span class="icon-play-btn">▶</span>
      </div>
    </article>
  </div>
{/snippet}
</Slop>
