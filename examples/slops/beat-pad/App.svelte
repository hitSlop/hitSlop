<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Button } from "bits-ui";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Shuffle from "@lucide/svelte/icons/shuffle";
  import Eraser from "@lucide/svelte/icons/eraser";
  import doc from "./schema";
  import { BeatEngine } from "./engine";
  import { STEPS, alphabet, cycle, empty, kitLabel, kits, shuffle, trackLabel, tracks, type Pattern, type Track } from "./pattern";

  let engine = $state.raw<BeatEngine>();
  let playing = $state(false);
  let starting = $state(false);
  let problem = $state("");
  let step = $state<number | null>(null);
  let liveBpm = $state<number | null>(null);
  let liveSwing = $state<number | null>(null);

  const beat = $derived(doc.current);
  const bpm = $derived(liveBpm ?? beat.bpm);
  const swing = $derived(liveSwing ?? beat.swing);
  const pattern = $derived<Pattern>({ kick: beat.kick, snare: beat.snare, hat: beat.hat, bass: beat.bass });
  const halves = [0, 1];

  $effect(() => { engine?.setPattern(pattern); });
  $effect(() => { engine?.setTempo(bpm, swing); });
  $effect(() => { engine?.setKit(beat.kit); });

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
      engine ??= await BeatEngine.create(pattern, beat.kit, (next) => (step = next));
      engine.setPattern(pattern);
      engine.setTempo(bpm, swing);
      engine.start();
      playing = true;
    } catch {
      problem = "Audio couldn’t start. Try Play again.";
    } finally {
      starting = false;
    }
  }

  const pause = () => { if (playing) { engine?.stop(); playing = false; } };
  onMount(() => {
    const hidden = () => { if (document.hidden) pause(); };
    document.addEventListener("visibilitychange", hidden);
    return () => document.removeEventListener("visibilitychange", hidden);
  });
  onDestroy(() => engine?.dispose());

  function press(track: Track, index: number) {
    doc.fields[track].set(cycle(track, pattern[track], index));
  }
  function randomise() {
    const next = shuffle();
    doc.change((tx) => { for (const track of tracks) tx.fields[track].set(next[track]); });
  }
  function clear() {
    const next = empty();
    doc.change((tx) => { for (const track of tracks) tx.fields[track].set(next[track]); });
  }
  function label(track: Track, index: number): string {
    const value = pattern[track][index] ?? ".";
    return `${trackLabel[track]} step ${index + 1}: ${value === "." ? "off" : track === "bass" ? `note ${value}` : "on"}`;
  }
  function keys(event: KeyboardEvent) {
    if (event.code !== "Space" || event.target instanceof HTMLInputElement || event.target instanceof HTMLButtonElement) return;
    event.preventDefault();
    void toggle();
  }
</script>

<svelte:window onkeydown={keys} />

<main class="studio" data-slop-selection="none" aria-label="Beat pad">
  <header class="top">
    <Button.Root class="play" aria-pressed={playing} aria-label={playing ? "Stop" : "Play"} disabled={starting} onclick={toggle}>
      {#if playing}<Square size={20} />{:else}<Play size={20} />{/if}
    </Button.Root>
    <h1>Beat Pad</h1>
    <p class="clock" aria-live="polite">{bpm} <small>BPM</small></p>
  </header>
  {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

  <section class="knobs" aria-label="Tempo and feel">
    <label>Tempo
      <input type="range" min="60" max="200" value={beat.bpm} oninput={(event) => (liveBpm = Number(event.currentTarget.value))} onchange={async (event) => { await doc.fields.bpm.set(Number(event.currentTarget.value)); liveBpm = null; }} />
    </label>
    <label>Swing {swing}%
      <input type="range" min="0" max="60" value={beat.swing} oninput={(event) => (liveSwing = Number(event.currentTarget.value))} onchange={async (event) => { await doc.fields.swing.set(Number(event.currentTarget.value)); liveSwing = null; }} />
    </label>
  </section>

  <div class="row-bar">
    <div class="kits" role="group" aria-label="Sound kit">
      {#each kits as kit}
        <Button.Root class="seg" aria-pressed={beat.kit === kit} onclick={() => doc.fields.kit.set(kit)}>{kitLabel[kit]}</Button.Root>
      {/each}
    </div>
    <Button.Root class="seg ghost" onclick={randomise}><Shuffle size={16} />Shuffle</Button.Root>
    <Button.Root class="seg ghost" onclick={clear}><Eraser size={16} />Clear</Button.Root>
  </div>

  <div class="grid" role="group" aria-label="Pattern">
    {#each halves as half}
      <div class="bar-block" data-half={half}>
        {#each tracks as track}
          <div class="track" data-track={track}>
            <span class="name">{trackLabel[track]}</span>
            {#each Array.from({ length: STEPS / 2 }, (_, i) => half * (STEPS / 2) + i) as index (index)}
              {@const value = pattern[track][index] ?? "."}
              <button
                class="cell"
                data-on={value !== "."}
                data-now={step === index}
                data-beat={index % 4 === 0}
                aria-pressed={value !== "."}
                aria-label={label(track, index)}
                onclick={() => press(track, index)}
              >{#if value !== "."}{track === "bass" ? value : "●"}{/if}</button>
            {/each}
          </div>
        {/each}
      </div>
    {/each}
  </div>
  <p class="hint">Tap a step to toggle it. Bass steps cycle through five notes. Space plays and stops.</p>
</main>
