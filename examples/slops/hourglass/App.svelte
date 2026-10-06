<script lang="ts">
import { bindText } from "hitslop/svelte";
import { prefersReducedMotion } from "svelte/motion";
import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
import CalendarClock from "@lucide/svelte/icons/calendar-clock";
import doc from "./schema";
import * as actions from "./commands";
import Vessel from "./Vessel.svelte";
import { localInput, presets, read } from "./model";

const day = 24 * 60 * 60_000;
let now = $state(Date.now());
const reading = $derived(read(doc.current, now));
const running = $derived(reading.state === "running");
/** The person asked to change the time; an unset glass always asks. */
let changing = $state(false);
const choosing = $derived(changing || reading.state === "unset");
/** The sand shown while the glass turns over: the amount it had before the turn. */
let turning = $state<number | null>(null);
let custom = $state(localInput(Date.now() + day));
let problem = $state("");
let visible = $state(document.visibilityState === "visible");

// The clock only ticks while sand is falling in a window someone can see.
$effect(() => {
  if (!running || !visible) return;
  const timer = setInterval(() => (now = Date.now()), 1000);
  return () => clearInterval(timer);
});
/** A window shown again catches up at once rather than on the next tick. */
function visibilityChanged() {
  visible = document.visibilityState === "visible";
  if (visible) now = Date.now();
}

/** Turns the glass over: it runs from now until `end`. */
async function turnOver(end: number) {
  if (!prefersReducedMotion.current) turning = reading.remaining;
  changing = false;
  problem = "";
  const at = Date.now();
  now = at;
  try { await actions.startUntil({ end }); }
  catch (error) { problem = error instanceof Error ? error.message : String(error); changing = true; }
}
async function again() {
  if (!prefersReducedMotion.current) turning = reading.remaining;
  now = Date.now();
  try { await actions.restart(); }
  catch (error) { problem = error instanceof Error ? error.message : String(error); }
}
function change() {
  const { end } = doc.current;
  custom = localInput(end > Date.now() ? end : Date.now() + day);
  problem = "";
  changing = true;
}
function until(event: SubmitEvent) {
  event.preventDefault();
  const end = new Date(custom).getTime();
  if (!custom || Number.isNaN(end)) problem = "Pick a date and time.";
  else if (end <= Date.now()) problem = "Pick a time that hasn't passed.";
  else void turnOver(end);
}
</script>

<svelte:document onvisibilitychange={visibilityChanged} />

<main class="hourglass" data-turning={turning !== null}>
  <div class="hourglass-vessel" data-turning={turning !== null} onanimationend={() => (turning = null)}>
    <Vessel
      layer="glass"
      remaining={turning ?? reading.remaining}
      running={turning === null && running}
      quiet={choosing && turning === null}
    />
  </div>
  <div class="hourglass-stand"><Vessel layer="frame" remaining={0} /></div>

  <input
    class="hourglass-title"
    aria-label="What it counts down to"
    placeholder="Name it"
    use:bindText={doc.fields.title}
  />

  {#if choosing}
    <section class="hourglass-presets" aria-labelledby="hourglass-presets-label">
      <p id="hourglass-presets-label">Turn it over for</p>
      <div>
        {#each presets as preset}
          <button type="button" onclick={() => turnOver(Date.now() + preset.duration)}>{preset.label}</button>
        {/each}
      </div>
    </section>
    <form class="hourglass-until" onsubmit={until}>
      <label for="hourglass-until-input">or until</label>
      <input id="hourglass-until-input" type="datetime-local" bind:value={custom} />
      <button type="submit">Start</button>
      <p aria-live="polite">{problem}</p>
    </form>
  {:else}
    <section class="hourglass-readout" role="timer" aria-label={doc.current.title || "Countdown"}>
      {#if reading.state === "running"}
        <strong>{reading.value}</strong>
        <span class="hourglass-unit">{reading.unit}</span>
        <span class="hourglass-when">until {reading.until}</span>
      {:else if reading.state === "done"}
        <strong class="hourglass-done">Time's up</strong>
        <span class="hourglass-when">since {reading.until}</span>
      {/if}
    </section>
  {/if}

  <div class="hourglass-actions">
    {#if choosing}
      {#if reading.state !== "unset"}<button type="button" onclick={() => (changing = false)}>Cancel</button>{/if}
    {:else}
      <button type="button" onclick={again}><RotateCcw size={15} aria-hidden="true" /> Turn over</button>
      <button type="button" onclick={change}><CalendarClock size={15} aria-hidden="true" /> Change</button>
    {/if}
  </div>
</main>
