<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Dialog, AlertDialog, Checkbox, Progress, Select } from "bits-ui";
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import schema, { restPresets, type Exercise, type RestPreset } from "./schema";
  import { checked, sets, complete, count, toggled, nextSet, nextExercise, formatTime } from "./workout";
  import { createRestClock } from "./rest";

  const PRESETS = restPresets.map((value) => ({
    value,
    label: Number(value) < 120 ? `${value}s` : `${Number(value) / 60}m`,
  }));

  const doc = useDocument(schema);
  const total = $derived(doc.current.exercises.reduce((sum, ex) => sum + sets(ex), 0));
  const done = $derived(doc.current.exercises.reduce((sum, ex) => sum + checked(ex).length, 0));
  const finished = $derived(total > 0 && done === total);
  let selectedId = $state<string | null>(null);
  const active = $derived(
    doc.current.exercises.find((ex) => ex.$id === (selectedId ?? nextExercise(doc.current.exercises))) ??
    doc.current.exercises.find((ex) => !complete(ex)) ??
    doc.current.exercises[0],
  );
  let contentEl = $state<HTMLDivElement>();
  let routine = $state(false);
  let reviewCompleted = $state(false);
  let dialogOpen = $state(false);
  let resetOpen = $state(false);
  let draft = $state<Draft | null>(null);
  let existing = $state(false);
  let formError = $state("");
  let announcement = $state("");
  const rest = createRestClock();
  let restSeconds = $state(0);
  let restTotal = $state(0);
  let restRunning = $state(false);
  let restFor = $state("");
  let pendingAdvance: string | null = null;

  type Draft = {
    id: string | null;
    name: string;
    sets: number;
    reps: number;
    weight: string;
  };

  function isPreset(value: string): value is RestPreset {
    return restPresets.some((preset) => preset === value);
  }

  function preset() {
    const value = Number(doc.current.restPreset);
    return value > 0 ? count(value, 1, 3600) : 90;
  }

  function syncRest() {
    const value = rest.read();
    restSeconds = value.seconds;
    restTotal = value.total;
    if (restRunning && value.seconds === 0) finishRest();
  }

  function finishRest() {
    const advance = pendingAdvance;
    rest.stop();
    restRunning = false;
    restSeconds = 0;
    restTotal = 0;
    pendingAdvance = null;
    if (advance) {
      selectedId = nextExercise(doc.current.exercises, advance);
      if (contentEl) contentEl.scrollTop = 0;
    }
    announcement = "Rest finished. Ready for your next set.";
  }

  function stopRest() {
    rest.stop();
    restRunning = false;
    restSeconds = 0;
    restTotal = 0;
    pendingAdvance = null;
  }

  function startRest(ex?: Exercise) {
    rest.start(preset());
    restRunning = true;
    restFor = ex?.name ?? active?.name ?? "";
    pendingAdvance = ex && complete(ex) ? ex.$id : null;
    syncRest();
    announcement = "Rest started.";
  }

  onMount(() => {
    const timer = setInterval(syncRest, 250);
    const visibility = () => {
      if (document.visibilityState === "visible") syncRest();
    };
    document.addEventListener("visibilitychange", visibility);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", visibility);
    };
  });

  onDestroy(() => {
    rest.stop();
  });

  function select(ex: Exercise) {
    selectedId = ex.$id;
    pendingAdvance = null;
    reviewCompleted = true;
    if (contentEl) contentEl.scrollTop = 0;
  }

  function toggleSet(ex: Exercise, index: number) {
    const next = toggled(ex, index);
    doc.change((tx) => {
      const handle = tx.at(ex);
      handle.completedSetIndices.replace(next.indices);
      handle.completedSets.set(next.indices.length);
    });
    selectedId = ex.$id;
    reviewCompleted = false;
    const updated = doc.current.exercises.find((item) => item.$id === ex.$id) ?? ex;
    if (doc.current.exercises.every(complete)) {
      stopRest();
      announcement = "Workout complete.";
    } else if (next.added) startRest(updated);
    else {
      stopRest();
      announcement = `Set ${index + 1} marked unfinished.`;
    }
  }

  function logSet() {
    if (!active) return;
    const index = nextSet(active);
    if (index !== undefined) toggleSet(active, index);
  }

  function edit(ex?: Exercise) {
    existing = !!ex;
    draft = ex
      ? { id: ex.$id, name: ex.name, sets: sets(ex), reps: ex.reps, weight: ex.weight }
      : { id: null, name: "", sets: 3, reps: 10, weight: "" };
    formError = "";
    dialogOpen = true;
  }

  function saveExercise() {
    if (!draft) return;
    const name = draft.name.trim();
    if (!name) {
      formError = "Give this exercise a name.";
      return;
    }
    if (!Number.isFinite(draft.sets) || !Number.isFinite(draft.reps)) {
      formError = "Enter a set and rep target.";
      return;
    }
    const reps = count(draft.reps, 1, 50);
    const weight = draft.weight.trim() || "Bodyweight";
    const setCount = count(draft.sets);
    if (draft.id) {
      const original = doc.current.exercises.find((ex) => ex.$id === draft!.id);
      if (!original) return;
      const indices = checked(original).filter((index) => index < setCount);
      doc.change((tx) => {
        const handle = tx.at(original);
        handle.name.replace(name);
        handle.sets.set(setCount);
        handle.reps.set(reps);
        handle.weight.set(weight);
        handle.completedSetIndices.replace(indices);
        handle.completedSets.set(indices.length);
      });
      if (pendingAdvance === original.$id && indices.length !== setCount) pendingAdvance = null;
    } else {
      doc.change((tx) => {
        tx.fields.exercises.insert({
          name,
          sets: setCount,
          reps,
          weight,
          completedSets: 0,
          completedSetIndices: [],
        });
      });
    }
    if (doc.current.exercises.every(complete)) stopRest();
    dialogOpen = false;
  }

  function removeExercise() {
    if (!draft?.id) return;
    const id = draft.id;
    doc.fields.exercises.remove(id);
    if (selectedId === id) selectedId = null;
    stopRest();
    dialogOpen = false;
  }

  function reorder(index: number, step: number) {
    const exercises = doc.current.exercises;
    const next = index + step;
    const item = exercises[index];
    const neighbor = exercises[next];
    if (!item || !neighbor || next < 0 || next >= exercises.length) return;
    doc.fields.exercises.move(item.$id, step < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
  }

  function resetProgress() {
    const exercises = doc.current.exercises;
    doc.change((tx) => {
      for (const ex of exercises) {
        const handle = tx.at(ex);
        handle.completedSets.set(0);
        handle.completedSetIndices.replace([]);
      }
    });
    selectedId = null;
    reviewCompleted = false;
    stopRest();
    announcement = "Progress reset. Ready to train.";
    resetOpen = false;
  }

  function exerciseNumber(ex: Exercise) {
    return String(doc.current.exercises.indexOf(ex) + 1).padStart(2, "0");
  }
</script>

<Slop>
  <main class="board" aria-label="Workout planner">
    <header class="header">
      <div class="top">
        <span class="eyebrow">Training / {routine ? "Build your routine" : "One set at a time"}</span>
        <button class="button" onclick={() => (routine = !routine)}>{routine ? "Back to workout" : "Edit routine"}</button>
      </div>
      <div class="top">
        <h1 class="title">{doc.current.title || "Your workout"}</h1>
        <span class="fraction">{done} / {total} sets</span>
      </div>
      <Progress.Root class="progress" value={done} max={Math.max(1, total)} aria-label="Workout completion">
        <div class="fill" style:width={`${total ? (done / total) * 100 : 0}%`}></div>
      </Progress.Root>
    </header>
    <div class="content" bind:this={contentEl}>
      {#if routine}
        <label class="field">Workout title<input class="input" use:bindText={doc.fields.title} /></label>
        <div class="section-head">
          <span class="eyebrow">Exercise order</span>
          <button class="button" onclick={() => edit()}>+ Add exercise</button>
        </div>
        <ul class="list">
          {#each doc.current.exercises as ex, index (ex.$id)}
            <li class="row">
              <button class="queue" aria-label="Edit {ex.name}" onclick={() => edit(ex)}>
                <span class="index">{String(index + 1).padStart(2, "0")}</span>
                <span>
                  <span class="queue-name">{ex.name}</span>
                  <span class="meta">{sets(ex)} × {ex.reps} · {ex.weight}</span>
                </span>
              </button>
              <div class="tools">
                <button class="button" aria-label="Move {ex.name} up" disabled={index === 0} onclick={() => reorder(index, -1)}>↑</button>
                <button class="button" aria-label="Move {ex.name} down" disabled={index === doc.current.exercises.length - 1} onclick={() => reorder(index, 1)}>↓</button>
              </div>
            </li>
          {/each}
        </ul>
        {#if total > 0}
          <div class="actions">
            <button class="button danger" onclick={() => (resetOpen = true)}>Reset progress</button>
          </div>
        {/if}
      {:else if finished && !reviewCompleted}
        <section class="empty">
          <div class="complete-mark" aria-hidden="true">✓</div>
          <span class="eyebrow">Session finished</span>
          <h2 class="exercise-name">Workout complete.</h2>
          <p class="description">{done} sets. All yours.</p>
          <button class="button primary" onclick={() => (resetOpen = true)}>Reset progress</button>
        </section>
      {:else if active}
        <section class="active" aria-label="Current exercise">
          <div class="kicker">
            <span>Exercise {exerciseNumber(active)} / {String(doc.current.exercises.length).padStart(2, "0")}</span>
            <span>{complete(active) ? "Sets complete" : `${checked(active).length} of ${sets(active)} sets`}</span>
          </div>
          <h2 class="exercise-name">{active.name}</h2>
          <div class="readouts">
            <div class="readout">
              <strong class="number">{active.reps}</strong>
              <span class="small">Reps per set</span>
            </div>
            <div class="readout">
              <strong class="number">{active.weight || "Bodyweight"}</strong>
              <span class="small">Target load</span>
            </div>
          </div>
          <div class="ticks" aria-label="Sets for {active.name}">
            {#each Array.from({ length: sets(active) }, (_, index) => index) as index}
              <Checkbox.Root
                class="tick"
                checked={checked(active).includes(index)}
                aria-label="Set {index + 1} of {active.name}"
                onCheckedChange={() => toggleSet(active, index)}
              >{checked(active).includes(index) ? "✓" : index + 1}</Checkbox.Root>
            {/each}
          </div>
          <button class="button primary" onclick={logSet} disabled={complete(active)}>
            {complete(active) ? "Exercise complete" : `Log set ${(nextSet(active) ?? 0) + 1}`}
          </button>
        </section>
      {:else}
        <section class="empty">
          <span class="eyebrow">Your starting point</span>
          <h2 class="exercise-name">Build your workout.</h2>
          <p class="description">Add an exercise and a target. Then take it one set at a time.</p>
          <button class="button primary" onclick={() => edit()}>+ Add your first exercise</button>
        </section>
      {/if}
      {#if !routine && total > 0}
        <div class="section-head">
          <span class="eyebrow">Your routine</span>
          <span class="meta">{doc.current.exercises.length} exercises</span>
        </div>
        <ul class="list">
          {#each doc.current.exercises as ex, index (ex.$id)}
            <li class="row">
              <button class="queue" aria-current={ex.$id === active?.$id ? "true" : undefined} aria-label="Select {ex.name}" onclick={() => select(ex)}>
                <span class="index">{complete(ex) ? "✓" : String(index + 1).padStart(2, "0")}</span>
                <span>
                  <span class="queue-name">{ex.name}</span>
                  <span class="meta">{sets(ex)} × {ex.reps} · {ex.weight}</span>
                </span>
                <span class="fraction">{checked(ex).length}/{sets(ex)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <section class="dock" aria-label="Rest timer">
      <div class="dock-top">
        <div>
          <span class="eyebrow">{restRunning ? "Rest / recover" : finished ? "Session complete" : "Rest timer"}</span>
          <div class="rest-time">
            {restRunning ? formatTime(restSeconds) : finished ? "NICE WORK" : formatTime(preset())}
          </div>
        </div>
        <div class="controls">
          {#if restRunning}
            <button class="button" onclick={() => { rest.extend(30); syncRest(); }}>+30s</button>
            <button class="button" onclick={finishRest}>Skip</button>
          {:else}
            <Select.Root
              type="single"
              value={doc.current.restPreset}
              items={PRESETS}
              onValueChange={(value) => { if (value && isPreset(value)) doc.fields.restPreset.set(value); }}
            >
              <Select.Trigger class="button" aria-label="Rest duration">
                {PRESETS.find((item) => item.value === doc.current.restPreset)?.label ?? `${preset()}s`} ▾
              </Select.Trigger>
              <Select.Portal>
                <Select.Content class="select-content" side="top" sideOffset={6}>
                  <Select.Viewport>
                    {#each PRESETS as item}
                      <Select.Item class="option" value={item.value} label={item.label}>{item.label}</Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
            <button class="button" disabled={finished || !total} onclick={() => startRest()}>Start</button>
          {/if}
        </div>
      </div>
      <Progress.Root class="progress" value={restSeconds} max={Math.max(1, restTotal)} aria-label="Rest remaining">
        <div class="fill" style:width={`${restTotal ? (restSeconds / restTotal) * 100 : 0}%`}></div>
      </Progress.Root>
      <p class="status">{restRunning ? `After ${restFor}` : "Starts automatically after each set."}</p>
    </section>
    <span role="status" style="position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)">{announcement}</span>
  </main>

  <Dialog.Root bind:open={dialogOpen}>
    <Dialog.Portal>
      <Dialog.Overlay class="overlay" />
      <Dialog.Content class="dialog">
        <Dialog.Title class="dialog-title">{existing ? "Edit exercise" : "Add an exercise"}</Dialog.Title>
        <Dialog.Description class="description">Set your target. Make it yours.</Dialog.Description>
        {#if draft}
          <form onsubmit={(event) => { event.preventDefault(); saveExercise(); }}>
            <div class="fields">
              <label class="field">Exercise name<input class="input" aria-label="Exercise name" bind:value={draft.name} required /></label>
              <div class="pair">
                <label class="field">Sets<input class="input" type="number" min="1" max="12" step="1" aria-label="Target sets" bind:value={draft.sets} required /></label>
                <label class="field">Reps per set<input class="input" type="number" min="1" max="50" step="1" aria-label="Target reps" bind:value={draft.reps} required /></label>
              </div>
              <label class="field">Weight / load<input class="input" aria-label="Weight" bind:value={draft.weight} placeholder="Bodyweight, 20 kg…" /></label>
            </div>
            {#if formError}<p class="error" role="alert">{formError}</p>{/if}
            <div class="actions">
              {#if existing}<button class="button danger" type="button" onclick={removeExercise}>Delete</button>{/if}
              <Dialog.Close class="button" type="button">Cancel</Dialog.Close>
              <button class="button" type="submit">Save exercise</button>
            </div>
          </form>
        {/if}
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  <AlertDialog.Root bind:open={resetOpen}>
    <AlertDialog.Portal>
      <AlertDialog.Overlay class="overlay" />
      <AlertDialog.Content class="dialog">
        <AlertDialog.Title class="dialog-title">Ready for another round?</AlertDialog.Title>
        <AlertDialog.Description class="description">Clear all completed sets and stop the rest timer. Your exercises and targets stay here.</AlertDialog.Description>
        <div class="actions">
          <AlertDialog.Cancel class="button">Keep progress</AlertDialog.Cancel>
          <AlertDialog.Action class="button" onclick={resetProgress}>Reset progress</AlertDialog.Action>
        </div>
      </AlertDialog.Content>
    </AlertDialog.Portal>
  </AlertDialog.Root>

  {#snippet exportView()}
    <article class="export-board" aria-label="Workout summary">
      <span class="eyebrow">Training / {done} of {total} sets</span>
      <h1 class="exercise-name">{doc.current.title || "Your workout"}</h1>
      <div class="progress">
        <div class="fill" style:width={`${total ? (done / total) * 100 : 0}%`}></div>
      </div>
      {#each doc.current.exercises as ex}
        <section class="export-lift">
          <h2 class="export-title">{ex.name}</h2>
          <p class="description">{sets(ex)} sets × {ex.reps} reps · {ex.weight || "Bodyweight"}</p>
          <div class="ticks">
            {#each Array.from({ length: sets(ex) }, (_, index) => index) as index}
              <span class="tick" data-state={checked(ex).includes(index) ? "checked" : "unchecked"} aria-label="Set {index + 1}: {checked(ex).includes(index) ? 'complete' : 'unfinished'}">{checked(ex).includes(index) ? "✓" : index + 1}</span>
            {/each}
          </div>
        </section>
      {:else}
        <p class="description">No exercises yet.</p>
      {/each}
    </article>
  {/snippet}

  {#snippet icon()}
    {@const marks = total ? Math.round((done / total) * 4) : 0}
    <svg class="icon" viewBox="0 0 512 512" fill="none" aria-hidden="true">
      <rect x="24" y="30" width="464" height="458" rx="64" fill="#050607" />
      <rect x="24" y="22" width="464" height="458" rx="64" fill="#191c1f" stroke="#343b3e" stroke-width="3" />
      <path d="M85 75h110" stroke="#c3f653" stroke-width="9" stroke-linecap="round" />
      <circle cx="256" cy="242" r="139" fill="#080a0b" stroke="#343b3e" stroke-width="8" />
      <circle cx="256" cy="236" r="117" fill="#282d30" stroke="#464e50" stroke-width="3" />
      <circle cx="256" cy="236" r="81" fill="#141719" stroke="#080a0b" stroke-width="12" />
      <circle cx="256" cy="236" r="32" fill="#080a0b" stroke="#66705c" stroke-width="7" />
      {#each [0, 120, 240] as angle}
        <rect x="230" y="134" width="52" height="27" rx="13" transform={`rotate(${angle} 256 236)`} fill="#080a0b" stroke="#59644f" stroke-width="3" />
      {/each}
      {#each [0, 1, 2, 3] as index}
        <rect x={100 + index * 82} y="402" width="66" height="28" rx="7" fill={index < marks ? "#c3f653" : "#303739"} stroke={index < marks ? "#c3f653" : "#505a53"} stroke-width="2" />
      {/each}
    </svg>
  {/snippet}
</Slop>
