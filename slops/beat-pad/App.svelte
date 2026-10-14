<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { getDocument } from "hitslop/svelte";
  import { onDestroy, onMount } from "svelte";
  import { Button } from "bits-ui";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Shuffle from "@lucide/svelte/icons/shuffle";
  import Eraser from "@lucide/svelte/icons/eraser";
  import schema from "./schema";
  import { BeatEngine } from "./engine";
  import { BPM, SWING, beatView, clearPattern, kitLabel, kits, pressStep, setKit, setSwing, setTempo, shufflePattern, type Track } from "./pattern";

  const doc = getDocument(schema);

  // The engine, Play state and playhead are view state: never saved, rebuilt from the pattern on Play.
  let engine = $state.raw<BeatEngine>();
  let playing = $state(false);
  let starting = $state(false);
  let problem = $state("");
  let step = $state<number | null>(null);
  let liveBpm = $state<number | null>(null);
  let liveSwing = $state<number | null>(null);

  const beat = $derived(doc.current);
  const view = $derived(beatView(beat));
  const pattern = $derived(view.pattern);
  const bpm = $derived(liveBpm ?? beat.bpm);
  const swing = $derived(liveSwing ?? beat.swing);

  // These push document state into the audio engine; they never write the document.
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
    } catch (error) {
      console.error("Beat Pad audio startup failed", error);
      problem = `Audio couldn’t start. ${error instanceof Error ? error.message : String(error)}`;
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

  const press = (track: Track, index: number) => doc.change((tx) => pressStep(tx, doc.current, track, index));
  const randomise = () => doc.change((tx) => shufflePattern(tx, Math.random));
  const clear = () => doc.change((tx) => clearPattern(tx));

  async function commitBpm(value: number) {
    await doc.change((tx) => setTempo(tx, value));
    liveBpm = null;
  }
  async function commitSwing(value: number) {
    await doc.change((tx) => setSwing(tx, value));
    liveSwing = null;
  }
  function keys(event: KeyboardEvent) {
    if (event.code !== "Space" || event.target instanceof HTMLInputElement || event.target instanceof HTMLButtonElement) return;
    event.preventDefault();
    void toggle();
  }
</script>

<svelte:window onkeydown={keys} />

<main class="studio" aria-label="Beat pad">
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
      <input type="range" min={BPM.min} max={BPM.max} value={beat.bpm} oninput={(event) => (liveBpm = Number(event.currentTarget.value))} onchange={(event) => commitBpm(Number(event.currentTarget.value))} />
    </label>
    <label>Swing {swing}%
      <input type="range" min={SWING.min} max={SWING.max} value={beat.swing} oninput={(event) => (liveSwing = Number(event.currentTarget.value))} onchange={(event) => commitSwing(Number(event.currentTarget.value))} />
    </label>
  </section>

  <div class="row-bar">
    <div class="kits" role="group" aria-label="Sound kit">
      {#each kits as kit}
        <Button.Root class="seg" aria-pressed={beat.kit === kit} onclick={() => doc.change((tx) => setKit(tx, kit))}>{kitLabel[kit]}</Button.Root>
      {/each}
    </div>
    <Button.Root class="seg ghost" onclick={randomise}><Shuffle size={16} />Shuffle</Button.Root>
    <Button.Root class="seg ghost" onclick={clear}><Eraser size={16} />Clear</Button.Root>
  </div>

  <div class="grid" role="group" aria-label="Pattern">
    {#each view.bars as bar (bar.half)}
      <div class="bar-block" data-half={bar.half}>
        {#each bar.tracks as row (row.track)}
          <div class="track" data-track={row.track}>
            <span class="name">{row.name}</span>
            {#each row.cells as cell (cell.index)}
              <button
                class="cell"
                data-on={cell.on}
                data-now={step === cell.index}
                data-beat={cell.beat}
                aria-pressed={cell.on}
                aria-label={cell.label}
                onclick={() => press(row.track, cell.index)}
              >{cell.mark}</button>
            {/each}
          </div>
        {/each}
      </div>
    {/each}
  </div>
  <p class="hint">Tap a step to toggle it. Bass steps cycle through five notes. Space plays and stops.</p>
</main>
