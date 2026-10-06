<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Button } from "bits-ui";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Volume from "@lucide/svelte/icons/volume-2";
  import VolumeOff from "@lucide/svelte/icons/volume-x";
  import type { Input } from "@hitslop/document";
  import doc, { maxTracks, voices } from "./schema";
  import type { LoopEngine, Sounds } from "./loops";

  type TrackInput = Input<typeof doc.descriptor>["tracks"][number];
  const presets = {
    drums: { kind: "drums", name: "Drums", code: "bd*2 sd", voice: "sine", gain: 0.8, filter: 9000, room: 0.1, muted: false },
    bass: { kind: "synth", name: "Bass", code: "c2 ~ eb2 ~", voice: "sawtooth", gain: 0.6, filter: 700, room: 0, muted: false },
    lead: { kind: "synth", name: "Lead", code: "c4 e4 g4 <b4 a4>", voice: "triangle", gain: 0.45, filter: 4000, room: 0.3, muted: false },
  } as const satisfies Record<string, TrackInput>;

  let engine = $state.raw<LoopEngine>();
  let playing = $state(false);
  let starting = $state(false);
  let problem = $state("");
  let sounds = $state<Sounds>("loading");
  let errors = $state<Record<string, string>>({});
  let liveBpm = $state<number | null>(null);
  let position = $state(0);

  const tracks = $derived(doc.current.tracks);
  const bpm = $derived(liveBpm ?? doc.current.bpm);
  const full = $derived(tracks.length >= maxTracks);

  // Edits take effect on the next cycle while it plays.
  $effect(() => {
    const snapshot = tracks;
    const tempo = bpm;
    if (playing && engine) void engine.play(snapshot, tempo).then((next) => (errors = next));
  });

  async function toggle() {
    if (playing) {
      engine?.stop();
      playing = false;
      return;
    }
    if (starting) return;
    starting = true;
    problem = "";
    try {
      if (!engine) {
        const { LoopEngine } = await import("./loops");
        engine = await LoopEngine.create((next) => (sounds = next));
      }
      errors = await engine.play(tracks, bpm);
      playing = true;
    } catch (error) {
      problem = error instanceof Error ? `Audio couldn’t start. ${error.message.slice(0, 100)}` : "Audio couldn’t start.";
    } finally {
      starting = false;
    }
  }

  onMount(() => {
    let frame = 0;
    const loop = () => {
      if (playing && engine) position = engine.position();
      frame = requestAnimationFrame(loop);
    };
    frame = requestAnimationFrame(loop);
    const hidden = () => { if (document.hidden && playing) { engine?.stop(); playing = false; } };
    document.addEventListener("visibilitychange", hidden);
    return () => { cancelAnimationFrame(frame); document.removeEventListener("visibilitychange", hidden); };
  });
  onDestroy(() => engine?.dispose());

  const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, value));
  const status = $derived(
    sounds === "ready" ? "Drum sounds ready" : sounds === "offline" ? "Offline: synths only" : engine ? "Loading drum sounds…" : "",
  );
</script>

<main class="lab" data-slop-selection="none" aria-label="Loop lab">
  <header class="top">
    <Button.Root class="play" aria-pressed={playing} aria-label={playing ? "Stop" : "Play"} disabled={starting} onclick={toggle}>
      {#if playing}<Square size={20} />{:else}<Play size={20} />{/if}
    </Button.Root>
    <div class="titles"><h1>Loop Lab</h1><p>{status || "Patterns in Strudel mini-notation"}</p></div>
    <p class="clock">{bpm} <small>BPM</small></p>
  </header>
  {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

  <div class="tempo">
    <label>Tempo
      <input type="range" min="60" max="180" value={doc.current.bpm} oninput={(event) => (liveBpm = Number(event.currentTarget.value))} onchange={async (event) => { await doc.fields.bpm.set(Number(event.currentTarget.value)); liveBpm = null; }} />
    </label>
    <div class="cycle" aria-hidden="true"><i style:width="{playing ? position * 100 : 0}%"></i></div>
  </div>

  <ul class="tracks">
    {#each tracks as track, i (track.$id)}
      {@const row = doc.at(track)}
      <li class="track" data-kind={track.kind} data-muted={track.muted}>
        <div class="line">
          <input class="name" aria-label={`Track ${i + 1} name`} maxlength="20" value={track.name} onchange={(event) => row.name.set(event.currentTarget.value.trim())} />
          {#if track.kind === "synth"}
            <select aria-label={`${track.name} voice`} value={track.voice} onchange={(event) => row.voice.set(event.currentTarget.value as (typeof voices)[number])}>
              {#each voices as voice}<option value={voice}>{voice}</option>{/each}
            </select>
          {:else}<span class="tag">samples</span>{/if}
          <button class="icon" aria-pressed={track.muted} aria-label={track.muted ? `Unmute ${track.name}` : `Mute ${track.name}`} onclick={() => row.muted.set(!track.muted)}>{#if track.muted}<VolumeOff size={18} />{:else}<Volume size={18} />{/if}</button>
          <button class="icon" aria-label={`Remove ${track.name}`} onclick={() => doc.fields.tracks.remove(track.$id)}><X size={18} /></button>
        </div>
        <input class="code" aria-label={`${track.name} pattern`} spellcheck="false" autocapitalize="off" autocomplete="off" maxlength="160" value={track.code} onchange={(event) => row.code.set(event.currentTarget.value.trim())} />
        {#if errors[track.$id]}<p class="error" role="status">Couldn’t read this pattern: {errors[track.$id]}</p>{/if}
        <div class="knobs">
          <label>Volume<input type="range" min="0" max="100" value={Math.round(track.gain * 100)} onchange={(event) => row.gain.set(clamp(Number(event.currentTarget.value) / 100, 0, 1))} /></label>
          <label>Filter<input type="range" min="200" max="12000" step="100" value={track.filter} onchange={(event) => row.filter.set(Number(event.currentTarget.value))} /></label>
          <label>Reverb<input type="range" min="0" max="100" value={Math.round(track.room * 100)} onchange={(event) => row.room.set(clamp(Number(event.currentTarget.value) / 100, 0, 1))} /></label>
        </div>
      </li>
    {/each}
  </ul>

  <div class="add-row" role="group" aria-label="Add a track">
    <span>Add</span>
    {#each Object.entries(presets) as [key, preset]}
      <Button.Root class="add" disabled={full} onclick={() => doc.fields.tracks.insert(preset)}><Plus size={16} />{key === "lead" ? "Lead" : key === "bass" ? "Bass" : "Drums"}</Button.Root>
    {/each}
  </div>
  <p class="cheat"><code>bd*2</code> repeat · <code>~</code> rest · <code>[a b]</code> group · <code>&lt;a b&gt;</code> alternate · <code>a(3,8)</code> euclid · <code>a?</code> sometimes · <code>,</code> layer</p>
</main>
