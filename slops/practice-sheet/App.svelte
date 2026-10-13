<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument, notify } from "hitslop/svelte";
  import type { Value } from "hitslop";
  import { Popover, Toggle } from "bits-ui";
  import { onMount, tick } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Repeat from "@lucide/svelte/icons/repeat";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Check from "@lucide/svelte/icons/check";
  import PencilLine from "@lucide/svelte/icons/pencil-line";
  import Music from "@lucide/svelte/icons/music";
  import Metronome from "@lucide/svelte/icons/metronome";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import Engraved from "./Engraved.svelte";
  import SheetHead from "./SheetHead.svelte";
  import Ladder from "./Ladder.svelte";
  import Record from "./Record.svelte";
  import { Player } from "./player";
  import { eventAt, Overlay, planPass, type Plan, type Score, type Span } from "./score";
  import {
    cleanRun, localDay, logPractice, MINUTES_MAX, practiceView, removeSession, restartLadder,
    setLadder, setTempo, shortDay, TEMPO_MAX, TEMPO_MIN,
  } from "./practice";

  const doc = getDocument(schema);

  // ---- view state: nothing below is saved
  let today = $state(localDay(new Date()));
  const view = $derived(practiceView(doc.current, today));

  let score = $state.raw<Score | null>(null);
  let overlay = $state.raw<Overlay | null>(null);
  let editing = $state(false);
  let playing = $state(false);
  let looping = $state(false);
  let span = $state<Span>({ from: 0, to: 3 });
  let clickOn = $state(true);
  let countInOn = $state(true);
  let status = $state("");
  let askClean = $state(false);
  let played = $state(0);
  let minutesDraft = $state<number | null>(null);
  let tempoDraft = $state<string | null>(null);
  let ladderDraft = $state({ start: 0, goal: 0, step: 0 });
  let ladderOpen = $state(false);
  let historyOpen = $state(false);

  const measures = $derived(score?.measures ?? 1);
  const activeSpan = $derived(looping ? span : null);
  const suggested = $derived(played >= 30 ? Math.min(MINUTES_MAX, Math.max(1, Math.round(played / 60))) : 15);
  const minutes = $derived(minutesDraft ?? suggested);

  const player = new Player();
  let frame = 0;
  let lastFrame = 0;
  let lastLine = -1;

  $effect(() => { overlay?.showSpan(score!, activeSpan); });

  function onscore(next: Score | null, element: HTMLElement) {
    if (playing) stop();
    score = next;
    overlay = next ? new Overlay(element) : null;
    if (next) {
      const last = next.measures - 1;
      const from = Math.min(span.from, last);
      span = { from, to: Math.min(Math.max(span.to, from), last) };
    }
  }

  function pick(measure: number, extend: boolean) {
    if (extend && looping) span = { from: Math.min(span.from, measure), to: Math.max(span.to, measure) };
    else span = { from: measure, to: measure };
    looping = true;
  }

  function setSpan(edge: "from" | "to", value: number) {
    if (!Number.isFinite(value)) return;
    const bar = Math.min(measures, Math.max(1, Math.round(value))) - 1;
    span = edge === "from" ? { from: bar, to: Math.max(bar, span.to) } : { from: Math.min(bar, span.from), to: bar };
  }

  // ---- playback
  function nextPass(source: Score): Plan | null {
    return looping ? planPass(source, span, doc.current.tempo) : null;
  }

  function play() {
    if (playing) { stop(); return; }
    if (!score) return;
    const source = score;
    player.start(planPass(source, activeSpan, doc.current.tempo), {
      countIn: countInOn,
      click: () => clickOn,
      next: () => nextPass(source),
      onEnd: () => finish(),
    });
    playing = true;
    askClean = false;
    lastFrame = performance.now();
    lastLine = -1;
    frame = requestAnimationFrame(animate);
  }

  function animate(now: number) {
    const position = player.position();
    if (!position || !score || !overlay) return;
    const { plan, seconds } = position;
    if (seconds < 0) {
      overlay.hideCursor();
      status = `Count-in · ${Math.ceil(-seconds / plan.scale)}`;
    } else {
      played += Math.min(0.25, (now - lastFrame) / 1000);
      const index = eventAt(score, plan, seconds);
      const event = score.events[index];
      const start = (event.ms - plan.startMs) * plan.scale / 1000;
      const end = index < plan.last ? (score.events[index + 1].ms - plan.startMs) * plan.scale / 1000 : plan.length;
      const fraction = prefersReducedMotion.current ? 0 : (seconds - start) / Math.max(0.001, end - start);
      overlay.showCursor(score, index, fraction);
      if (event.line !== lastLine) {
        lastLine = event.line;
        overlay.cursorElement.scrollIntoView({ block: "nearest", behavior: prefersReducedMotion.current ? "auto" : "smooth" });
      }
      status = `Bar ${event.measure + 1}${looping ? ` · pass ${player.passes + 1}` : ""}`;
    }
    lastFrame = now;
    frame = requestAnimationFrame(animate);
  }

  function stop() {
    player.stop();
    cancelAnimationFrame(frame);
    overlay?.hideCursor();
    playing = false;
    status = "";
  }

  function finish() {
    stop();
    askClean = !view.atGoal;
  }

  // ---- edits (each is written once in practice.ts)
  async function climb() {
    askClean = false;
    const next = await doc.change(tx => cleanRun(tx, doc.current));
    notify(next >= doc.current.ladder.goal ? `Goal reached: ${next} bpm.` : `Clean run. Up to ${next} bpm.`);
  }

  function nudge(direction: -1 | 1) {
    const next = Math.min(TEMPO_MAX, Math.max(TEMPO_MIN, doc.current.tempo + direction * doc.current.ladder.step));
    void doc.change(tx => setTempo(tx, next));
  }

  function commitTempo() {
    if (tempoDraft === null) return;
    const value = Number(tempoDraft);
    tempoDraft = null;
    if (value === doc.current.tempo) return;
    void doc.change(tx => setTempo(tx, value));
  }

  function openLadder(open: boolean) {
    ladderOpen = open;
    if (open) ladderDraft = { ...doc.current.ladder };
  }

  async function saveLadder(event: SubmitEvent) {
    event.preventDefault();
    await doc.change(tx => setLadder(tx, doc.current, ladderDraft));
    ladderOpen = false;
  }

  async function restart() {
    const bpm = await doc.change(tx => restartLadder(tx, doc.current));
    ladderOpen = false;
    notify(`Back to ${bpm} bpm.`);
  }

  async function log(event: SubmitEvent) {
    event.preventDefault();
    today = localDay(new Date());
    const logged = minutes;
    await doc.change(tx => logPractice(tx, doc.current, today, logged));
    minutesDraft = null;
    played = 0;
    notify(`Logged ${logged} min at ${doc.current.tempo} bpm.`);
  }

  async function remove(session: Value<typeof schema.descriptor>["log"][number]) {
    await doc.change(tx => removeSession(tx, session));
    notify("Session removed.");
  }

  // ---- lifecycle
  function shortcut(event: KeyboardEvent) {
    if (event.key !== " " || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("input, textarea, select, button, [contenteditable], [role=switch], [role=dialog]")) return;
    event.preventDefault();
    play();
  }

  onMount(() => {
    const visibility = () => {
      if (document.hidden && playing) stop();
      if (!document.hidden) today = localDay(new Date());
    };
    const clock = setInterval(() => (today = localDay(new Date())), 60_000);
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("keydown", shortcut);
    return () => {
      clearInterval(clock);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("keydown", shortcut);
      cancelAnimationFrame(frame);
      player.dispose();
    };
  });

  async function toggleEditing() {
    editing = !editing;
    await tick();
    if (editing) (document.querySelector(".ps-abc textarea") as HTMLTextAreaElement | null)?.focus();
  }
</script>

<main class="ps-desk">
  <section class="ps-stand" aria-label="Score">
    <article class="ps-sheet" data-editing={editing}>
      <header class="ps-head">
        <SheetHead header={view.header} />
        <button class="ps-quiet" onclick={toggleEditing} aria-pressed={editing}>
          {#if editing}<Check size={16} /> Done{:else}<PencilLine size={16} /> Edit notes{/if}
        </button>
      </header>
      {#if editing}
        <div class="ps-abc">
          <textarea
            use:bindText={doc.fields.abc}
            aria-label="ABC notation"
            spellcheck="false"
            autocapitalize="off"
            autocomplete="off"
          ></textarea>
          <p>ABC notation: header lines (T: title, M: meter, L: note length, K: key), then the notes. The score redraws as you type.</p>
        </div>
      {/if}
      <div class="ps-score">
        <Engraved abc={doc.current.abc} width={480} delay={250} {onscore} onpick={pick} />
        {#if !score}
          <div class="ps-unplayable">
            <Music size={28} />
            <p>Nothing to engrave yet. ABC needs a <code>K:</code> line and at least one note.</p>
          </div>
        {/if}
      </div>
    </article>

    <div class="ps-ledge" role="toolbar" aria-label="Playback">
      <button class="ps-play" onclick={play} disabled={!score} aria-label={playing ? "Stop" : "Play"} aria-keyshortcuts="Space">
        {#if playing}<Square size={20} fill="currentColor" />{:else}<Play size={22} fill="currentColor" />{/if}
      </button>
      <div class="ps-loop-group" data-on={looping}>
        <Toggle.Root bind:pressed={looping} class="ps-toggle" aria-label="Loop a passage">
          <Repeat size={17} /> Loop
        </Toggle.Root>
        <label class="ps-bar">bars
          <input type="number" min="1" max={measures} value={span.from + 1} disabled={!looping}
            aria-label="Loop from bar" onchange={(e) => setSpan("from", e.currentTarget.valueAsNumber)} />
        </label>
        <span aria-hidden="true">–</span>
        <label class="ps-bar">
          <input type="number" min="1" max={measures} value={span.to + 1} disabled={!looping}
            aria-label="Loop to bar" onchange={(e) => setSpan("to", e.currentTarget.valueAsNumber)} />
        </label>
      </div>
      <Toggle.Root bind:pressed={clickOn} class="ps-toggle" aria-label="Metronome click">
        <Metronome size={17} /> Click
      </Toggle.Root>
      <Toggle.Root bind:pressed={countInOn} class="ps-toggle" aria-label="One-bar count-in">
        1-bar count-in
      </Toggle.Root>
      <p class="ps-status" aria-live="polite">
        {#if askClean}
          <span class="ps-ask">Was that clean?
            <button onclick={climb}>Yes, +{view.ladder.step}</button>
            <button onclick={() => (askClean = false)}>Not yet</button>
          </span>
        {:else if status}{status}
        {:else if looping}Click a note to pick a bar · Shift-click to extend
        {:else}Space to play · click a note to loop its bar{/if}
      </p>
    </div>
  </section>

  <aside class="ps-rail" aria-label="Practice">
    <section class="ps-tempo" aria-label="Tempo">
      <div class="ps-tempo-read">
        <p class="ps-label">Tempo</p>
        <label class="ps-numeral">
          <span class="ps-sr">Tempo in beats per minute</span>
          <input
            inputmode="numeric"
            style:--digits={Math.max(2, (tempoDraft ?? String(view.tempo)).length)}
            value={tempoDraft ?? String(view.tempo)}
            oninput={(e) => (tempoDraft = e.currentTarget.value)}
            onchange={commitTempo}
            onkeydown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); if (e.key === "Escape") { tempoDraft = null; e.currentTarget.blur(); } }}
          />
          <span class="ps-unit" aria-hidden="true">bpm</span>
        </label>
        <div class="ps-nudge">
          <button onclick={() => nudge(-1)} aria-label={`Slow down ${view.ladder.step} bpm`}><Minus size={16} /></button>
          <button onclick={() => nudge(1)} aria-label={`Speed up ${view.ladder.step} bpm`}><Plus size={16} /></button>
        </div>
        <p class="ps-goal">
          {#if view.atGoal}At your goal{:else}Goal {view.ladder.goal} · {view.ladder.goal - view.tempo} to go{/if}
        </p>
      </div>
      <Ladder rungs={view.rungs} tempo={view.tempo} />
    </section>

    <div class="ps-climb">
      <button class="ps-clean" onclick={climb} disabled={view.atGoal}>
        <Check size={18} />
        {#if view.atGoal}Goal reached{:else}Clean run → {view.nextTempo}{/if}
      </button>
      <Popover.Root open={ladderOpen} onOpenChange={openLadder}>
        <Popover.Trigger class="ps-icon-button" aria-label="Ladder settings"><SlidersHorizontal size={18} /></Popover.Trigger>
        <Popover.Portal>
          <Popover.Content class="ps-pop" side="left" align="start" sideOffset={10}>
            <form onsubmit={saveLadder}>
              <p class="ps-pop-title">Tempo ladder</p>
              <div class="ps-fields">
                <label>Start <input type="number" min={TEMPO_MIN} max={TEMPO_MAX} bind:value={ladderDraft.start} /></label>
                <label>Goal <input type="number" min={TEMPO_MIN} max={TEMPO_MAX} bind:value={ladderDraft.goal} /></label>
                <label>Step <input type="number" min="1" max="40" bind:value={ladderDraft.step} /></label>
              </div>
              <p class="ps-pop-note">Each clean run adds the step, up to the goal.</p>
              <div class="ps-pop-actions">
                <button type="button" class="ps-quiet" onclick={restart}><RotateCcw size={15} /> Back to {doc.current.ladder.start}</button>
                <button type="submit" class="ps-solid">Save</button>
              </div>
            </form>
          </Popover.Content>
        </Popover.Portal>
      </Popover.Root>
    </div>

    <section class="ps-record" aria-label="Practice record">
      <header class="ps-record-head">
        <p class="ps-label">Practice record</p>
        <Popover.Root bind:open={historyOpen}>
          <Popover.Trigger class="ps-link"><ScrollText size={14} /> {view.sessions.length} sessions</Popover.Trigger>
          <Popover.Portal>
            <Popover.Content class="ps-pop ps-history" side="left" align="end" sideOffset={10}>
              <p class="ps-pop-title">Sessions</p>
              {#if view.sessions.length}
                <ol>
                  {#each view.sessions as session (session.$id)}
                    <li>
                      <span>{shortDay(session.at)}</span>
                      <span>{session.minutes} min</span>
                      <span>{session.tempo} bpm</span>
                      <button aria-label={`Remove ${shortDay(session.at)}, ${session.minutes} minutes`} onclick={() => remove(session)}><X size={14} /></button>
                    </li>
                  {/each}
                </ol>
              {:else}
                <p class="ps-pop-note">Log a session and it shows up here.</p>
              {/if}
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
      </header>
      <Record {view} />
      <form class="ps-log" onsubmit={log}>
        <label>
          <span class="ps-sr">Minutes practised</span>
          <input type="number" min="1" max={MINUTES_MAX} value={minutes}
            onchange={(e) => (minutesDraft = Number.isFinite(e.currentTarget.valueAsNumber) ? e.currentTarget.valueAsNumber : null)} />
          <span aria-hidden="true">min</span>
        </label>
        <button type="submit" class="ps-solid">Log today at {view.tempo}</button>
      </form>
    </section>
  </aside>
</main>
