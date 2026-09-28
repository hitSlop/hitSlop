<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { attachments } from "@hitslop/document/attachments";
  import { capture } from "@hitslop/document/capture";
  import { Dialog, Progress, Select, Checkbox } from "bits-ui";
  import { grow } from "./grow";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Clock from "@lucide/svelte/icons/clock";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import Camera from "@lucide/svelte/icons/camera";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import schema, { difficulties, type Step } from "./schema";

  type OptionalNumber = {
    value: number | undefined;
    min: number;
    max: number;
    set: (value: number) => void;
    clear: () => void;
  };

  function bindOptionalNumber(node: HTMLInputElement, binding: OptionalNumber) {
    let current = binding;
    let focused = false;
    const sync = () => {
      if (focused) return;
      const next = current.value === undefined ? "" : String(current.value);
      if (node.value !== next) node.value = next;
    };
    const commit = () => {
      const raw = node.value.trim();
      if (raw === "") {
        if (current.value !== undefined) current.clear();
        return;
      }
      const parsed = Number(raw);
      if (!Number.isFinite(parsed)) return sync();
      const next = Math.min(current.max, Math.max(current.min, parsed));
      if (next !== current.value) current.set(next);
      if (node.value !== String(next)) node.value = String(next);
    };
    const onFocus = () => { focused = true; };
    const onBlur = () => { focused = false; commit(); };
    node.addEventListener("focus", onFocus);
    node.addEventListener("change", commit);
    node.addEventListener("blur", onBlur);
    sync();
    return {
      update(next: OptionalNumber) {
        current = next;
        sync();
      },
      destroy() {
        node.removeEventListener("focus", onFocus);
        node.removeEventListener("change", commit);
        node.removeEventListener("blur", onBlur);
      },
    };
  }

  const difficultyItems = difficulties.map((value) => ({ value, label: value }));
  const doc = useDocument(schema);

  let cookingOpen = $state(false);
  let currentStepIndex = $state(0);
  let completedStepIDs = $state<string[]>([]);
  let remainingByStep = $state<Record<string, number>>({});
  let running = $state(false);
  let finished = $state(false);
  let announcement = $state("");
  let timer: ReturnType<typeof setInterval> | null = null;
  let deadline = 0;
  let tweenReady = false;
  let picker = $state<HTMLInputElement>();
  let photoUrl = $state("");
  let photoError = $state<string | null>(null);
  let photoWait: Promise<void> = Promise.resolve();

  const activeStep = $derived(doc.current.steps[currentStepIndex] ?? null);
  const timedStepCount = $derived(doc.current.steps.filter((step) => stepSeconds(step) > 0).length);
  const activeRemaining = $derived(activeStep ? remainingFor(activeStep) : 0);
  const activeDuration = $derived(activeStep ? stepSeconds(activeStep) : 0);
  const remainingRatio = $derived(activeDuration > 0 ? activeRemaining / activeDuration : 0);
  const remainingMotion = new Tween(untrack(() => remainingRatio), { duration: 250, easing: cubicOut });
  const timerLabel = $derived(formatTime(activeRemaining));
  const photo = $derived(doc.current.photo);

  $effect(() => {
    const current = photo;
    let cancelled = false;
    let url = "";
    if (!current) {
      photoUrl = "";
      photoWait = Promise.resolve();
      return;
    }
    photoWait = attachments.read(current.id, { type: current.mimeType }).then((blob) => {
      if (cancelled) return;
      url = URL.createObjectURL(blob);
      photoUrl = url;
    }).catch(() => {
      if (!cancelled) photoError = "That photo couldn’t be added. Try another image.";
    });
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  });
  $effect(() => {
    if (!cookingOpen && running) pauseTimer();
    if (!cookingOpen && finished) resetCookingSession();
  });
  $effect(() => {
    const instant = !tweenReady || prefersReducedMotion.current;
    void remainingMotion.set(remainingRatio, { duration: instant ? 0 : 250, delay: 0 });
    tweenReady = true;
  });
  onMount(() => capture.onPrepare(async () => {
    await photoWait;
    await remainingMotion.set(remainingRatio, { duration: 0, delay: 0 });
    await tick();
  }));
  onDestroy(() => {
    stopTick();
    void remainingMotion.set(remainingMotion.target, { duration: 0, delay: 0 });
  });

  function isDifficulty(value: string): value is (typeof difficulties)[number] {
    return difficulties.some((item) => item === value);
  }
  function stepSeconds(step: Step): number {
    return Math.max(0, Math.round(Number(step.minutes) || 0) * 60);
  }
  function remainingFor(step: Step): number {
    return remainingByStep[step.$id] ?? stepSeconds(step);
  }
  function setRemaining(stepID: string, seconds: number): void {
    remainingByStep = { ...remainingByStep, [stepID]: Math.max(0, seconds) };
  }
  function formatTime(seconds: number): string {
    return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
  }
  function stepTitle(step: Step, index = currentStepIndex): string {
    return step.title.trim() || `Step ${index + 1}`;
  }
  function addIngredient(): void {
    doc.fields.ingredients.insert({ text: "New ingredient", checked: false });
  }
  function moveRow(kind: "ingredients" | "steps", id: string, direction: -1 | 1): void {
    const items = doc.current[kind];
    const index = items.findIndex((item) => item.$id === id);
    const neighbor = items[index + direction];
    if (index < 0 || !neighbor) return;
    doc.fields[kind].move(id, direction < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
  }
  function addStep(): void {
    const number = doc.current.steps.length + 1;
    doc.fields.steps.insert({ title: `Step ${number}`, text: "Describe the next step." });
  }
  function removeStep(id: string): void {
    doc.fields.steps.remove(id);
    delete remainingByStep[id];
    remainingByStep = { ...remainingByStep };
    completedStepIDs = completedStepIDs.filter((stepID) => stepID !== id);
  }
  function stopTick(): void {
    if (timer) clearInterval(timer);
    timer = null;
  }
  function pauseTimer(): void {
    if (running && activeStep) setRemaining(activeStep.$id, Math.ceil((deadline - Date.now()) / 1000));
    running = false;
    stopTick();
  }
  function toggleTimer(): void {
    if (!activeStep || stepSeconds(activeStep) <= 0) return;
    if (running) { pauseTimer(); return; }
    const stepID = activeStep.$id;
    let remaining = remainingFor(activeStep);
    if (remaining <= 0) { remaining = stepSeconds(activeStep); setRemaining(stepID, remaining); }
    deadline = Date.now() + remaining * 1000;
    running = true;
    announcement = `${stepTitle(activeStep)} timer started.`;
    timer = setInterval(() => {
      const next = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
      setRemaining(stepID, next);
      if (next === 0) { running = false; stopTick(); announcement = `Time’s up for ${stepTitle(activeStep)}.`; }
    }, 250);
  }
  function resetTimer(): void {
    if (!activeStep) return;
    pauseTimer();
    setRemaining(activeStep.$id, stepSeconds(activeStep));
    announcement = `${stepTitle(activeStep)} timer reset.`;
  }
  function prepareCooking(): void {
    if (finished) resetCookingSession();
    currentStepIndex = Math.min(currentStepIndex, Math.max(0, doc.current.steps.length - 1));
    const step = doc.current.steps[currentStepIndex];
    if (step) announcement = `${stepTitle(step, currentStepIndex)} ready.`;
  }
  function goToStep(index: number): void {
    if (!doc.current.steps.length) return;
    pauseTimer();
    currentStepIndex = Math.max(0, Math.min(index, doc.current.steps.length - 1));
    const step = doc.current.steps[currentStepIndex];
    if (!step) return;
    announcement = `${stepTitle(step, currentStepIndex)}, step ${currentStepIndex + 1} of ${doc.current.steps.length}.`;
  }
  function nextStep(): void {
    if (!activeStep) return;
    pauseTimer();
    if (!completedStepIDs.includes(activeStep.$id)) completedStepIDs = [...completedStepIDs, activeStep.$id];
    if (currentStepIndex === doc.current.steps.length - 1) {
      finished = true;
      announcement = `${doc.current.title} is ready.`;
      return;
    }
    goToStep(currentStepIndex + 1);
  }
  function resetCookingSession(): void {
    running = false;
    stopTick();
    currentStepIndex = 0;
    completedStepIDs = [];
    remainingByStep = {};
    finished = false;
    announcement = "Cooking session reset.";
  }
  async function onPhoto(event: Event) {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    event.stopImmediatePropagation();
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    if (file.type && !file.type.startsWith("image/")) {
      photoError = "That photo couldn’t be added. Try another image.";
      return;
    }
    photoError = null;
    try {
      await attachments.import(file, { commit(ref) {
        doc.fields.photo.set({ id: ref.id, name: ref.name, mimeType: ref.mimeType });
      } });
    } catch {
      photoError = "That photo couldn’t be added. Try another image.";
    }
  }
</script>

{#snippet cookbook()}
  <svg class="iconBook" viewBox="0 0 512 512" fill="none">
    <path d="M32 110Q142 76 256 118Q370 76 480 110V402Q368 370 256 412Q144 370 32 402Z" fill="currentColor" />
    <path d="M53 91Q151 63 246 108V382Q145 345 53 373Z" fill="var(--slop-paper)" stroke="currentColor" stroke-width="12" stroke-linejoin="round" />
    <path d="M266 108Q361 63 459 91V373Q367 345 266 382Z" fill="var(--slop-paper)" stroke="currentColor" stroke-width="12" stroke-linejoin="round" />
    <path d="M343 86L393 86V228L368 207L343 228Z" fill="var(--slop-tomato)" />
    <g stroke="currentColor" stroke-width="12" stroke-linecap="round">
      <path d="M89 262L209 275M89 301L182 311M306 276L418 263M306 316L393 305" />
      <path d="M120 170H192V198Q156 225 120 198ZM114 159H198M151 141H161" />
    </g>
  </svg>
{/snippet}

<Slop>
  <main class="canvas" data-slop-selection="none" aria-label="Recipe card">
    <article class="card" aria-label="Recipe for {doc.current.title}">
      <header class="hero">
        <div class="intro">
          <div class="utilityLine">
            <Select.Root type="single" value={doc.current.difficulty} items={difficultyItems} onValueChange={(value) => { if (isDifficulty(value)) doc.fields.difficulty.set(value); }}>
              <Select.Trigger class="difficultyTrigger" aria-label="Difficulty">
                <Select.Value placeholder="Difficulty" />
                <ChevronDown size={11} strokeWidth={2.2} data-slop-export="hide" />
              </Select.Trigger>
              <Select.Portal>
                <Select.Content class="selectContent" sideOffset={6}>
                  <Select.Viewport>
                    {#each difficultyItems as item (item.value)}
                      <Select.Item value={item.value} label={item.label}>
                        {#snippet children({ selected })}{item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}{/snippet}
                      </Select.Item>
                    {/each}
                  </Select.Viewport>
                </Select.Content>
              </Select.Portal>
            </Select.Root>
          </div>
          <textarea rows="1" use:grow={doc.current.title} class="title" aria-label="Recipe title" placeholder="Recipe title" use:bindText={doc.fields.title}></textarea>
          <textarea class="description" aria-label="Recipe description" placeholder="A short note about the dish" use:bindText={doc.fields.description}></textarea>
          <div class="stats">
            <label><span>Serves</span><input aria-label="Servings" type="number" min="1" use:bindOptionalNumber={{ value: doc.current.servings, min: 1, max: 999, set: (value) => doc.fields.servings.set(value), clear: () => doc.fields.servings.clear() }} /></label>
            <label><span>Prep</span><span class="number"><input aria-label="Preparation minutes" type="number" min="0" use:bindOptionalNumber={{ value: doc.current.prepMinutes, min: 0, max: 9999, set: (value) => doc.fields.prepMinutes.set(value), clear: () => doc.fields.prepMinutes.clear() }} /><small>min</small></span></label>
            <label><span>Cook</span><span class="number"><input aria-label="Cooking minutes" type="number" min="0" use:bindOptionalNumber={{ value: doc.current.cookMinutes, min: 0, max: 9999, set: (value) => doc.fields.cookMinutes.set(value), clear: () => doc.fields.cookMinutes.clear() }} /><small>min</small></span></label>
          </div>
        </div>

        <figure class="photoWell" data-photo={photo ? "true" : "false"}>
          {#if photoUrl}<img src={photoUrl} alt={doc.current.title} />{/if}
          {#if !photo}<button class="photoInvitation" onclick={() => picker?.click()} data-slop-export="hide"><Camera size={25} strokeWidth={1.5} />Add meal photo</button>{/if}
          {#if photo}<div class="photoActions" data-slop-export="hide">
            <button type="button" aria-label="Replace meal photo" onclick={() => picker?.click()}>
              <Camera size={13} />Replace
            </button>
            <button type="button" class="photoRemove" aria-label="Remove meal photo" onclick={() => doc.fields.photo.clear()}><X size={13} /></button>
          </div>{/if}
          <input bind:this={picker} class="srOnly" data-slop-export="hide" type="file" accept="image/*" tabindex="-1" onchange={onPhoto} />
        </figure>
      </header>

      <Dialog.Root bind:open={cookingOpen}>
        <Dialog.Trigger class="cookingLaunch" data-slop-export="hide" disabled={doc.current.steps.length === 0} onclick={prepareCooking}>
          <span class="launchMark"><Play size={14} /></span>
          <span class="launchCopy"><strong>Cooking mode</strong><small>{doc.current.steps.length} {doc.current.steps.length === 1 ? "step" : "steps"} · {timedStepCount} {timedStepCount === 1 ? "timer" : "timers"}</small></span>
          <span class="launchAction">Start cooking <ArrowRight size={16} aria-hidden="true" /></span>
        </Dialog.Trigger>

        <Dialog.Portal>
          <Dialog.Overlay class="cookOverlay" data-slop-export="hide" />
          <Dialog.Content class="cookDialog" data-slop-export="hide">
            <header class="cookHeader">
              {#if photoUrl}<img src={photoUrl} alt="" />{:else}<span class="cookBookMark"><div class="iconSurface" aria-hidden="true">{@render cookbook()}</div></span>{/if}
              <div><span>NOW COOKING</span><strong>{doc.current.title}</strong></div>
              <Dialog.Close class="cookClose" aria-label="Close cooking mode" onclick={pauseTimer}><X size={14} /></Dialog.Close>
            </header>

            <Progress.Root
              class="cookProgress"
              value={finished ? doc.current.steps.length : currentStepIndex + 1}
              max={Math.max(1, doc.current.steps.length)}
              aria-label="Cooking progress"
              aria-valuetext={finished ? "Cooking complete" : `Step ${currentStepIndex + 1} of ${doc.current.steps.length}`}
            >
              {#each doc.current.steps as step, index (step.$id)}
                <span data-state={finished || completedStepIDs.includes(step.$id) ? "complete" : index === currentStepIndex ? "current" : "upcoming"}></span>
              {/each}
            </Progress.Root>

            {#if finished}
              <section class="cookFinished">
                <span class="finishedMark" aria-hidden="true"><Check size={30} /></span>
                <p class="stepKicker">ALL STEPS COMPLETE</p>
                <Dialog.Title class="cookingTitle">{doc.current.title} is ready.</Dialog.Title>
                <Dialog.Description class="cookingDescription">Bring it to the table while it’s at its best.</Dialog.Description>
                <div class="finishActions">
                  <button type="button" onclick={resetCookingSession}><RotateCcw size={13} />Cook again</button>
                  <Dialog.Close class="finishDone" onclick={resetCookingSession}>Done</Dialog.Close>
                </div>
              </section>
            {:else if activeStep}
              <section class="cookStage">
                <p class="stepKicker">STEP {String(currentStepIndex + 1).padStart(2, "0")} / {String(doc.current.steps.length).padStart(2, "0")}</p>
                <Dialog.Title class="cookingTitle">{stepTitle(activeStep)}</Dialog.Title>
                <Dialog.Description class="cookingDescription">{activeStep.text}</Dialog.Description>

                {#if stepSeconds(activeStep) > 0}
                  <div class="stepTimer" data-expired={activeRemaining === 0}>
                    <span class="timerFill" style:transform={`scaleX(${remainingMotion.current})`}></span>
                    <span class="timerCaption">{activeRemaining === 0 ? "TIME’S UP" : running ? "COUNTING DOWN" : "STEP TIMER"}</span>
                    <output aria-live="off">{timerLabel}</output>
                    <div>
                      <button class="timerReset" type="button" aria-label="Reset step timer" onclick={resetTimer}><RotateCcw size={13} /></button>
                      <button class="timerMain" type="button" aria-label={running ? "Pause step timer" : activeRemaining === 0 ? "Restart step timer" : "Start step timer"} onclick={toggleTimer}>
                        {#if running}<Pause size={13} />Pause{:else}<Play size={13} />{activeRemaining === 0 ? "Again" : "Start"}{/if}
                      </button>
                    </div>
                  </div>
                {:else}
                  <div class="untimedStep"><span aria-hidden="true"><Clock size={20} /></span><p><strong>No timer needed</strong>Move on when this step feels right.</p></div>
                {/if}
              </section>

              <nav class="cookNav" aria-label="Cooking steps">
                <button type="button" disabled={currentStepIndex === 0} onclick={() => goToStep(currentStepIndex - 1)}><ArrowLeft size={16} aria-hidden="true" /> Previous</button>
                <span>{currentStepIndex + 1} of {doc.current.steps.length}</span>
                <button type="button" class="cookNext" onclick={nextStep}>{currentStepIndex === doc.current.steps.length - 1 ? "Finish" : "Next"} <ArrowRight size={16} aria-hidden="true" /></button>
              </nav>
            {/if}
            <p class="srOnly" role="status" aria-live="polite">{announcement}</p>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <div class="body">
        <section class="section" aria-labelledby="ingredients-heading">
          <div class="sectionTitle"><div><h2 id="ingredients-heading">Ingredients</h2></div><button data-slop-export="hide" aria-label="Add ingredient" onclick={addIngredient}><Plus size={12} /></button></div>
          <ul class="list">
            {#each doc.current.ingredients as item, index (item.$id)}
              <li class="ingredientRow" data-checked={item.checked}>
                <Checkbox.Root checked={item.checked} onCheckedChange={(checked) => doc.at(item).checked.set(checked === true)} aria-label="Mark {item.text || 'untitled ingredient'} {item.checked ? 'still needed' : 'complete'}">
                  {#snippet children({ checked })}{#if checked}<Check size={10} strokeWidth={3} />{/if}{/snippet}
                </Checkbox.Root>
                <input class="itemCopy" aria-label="Ingredient {index + 1}" use:bindText={doc.at(item).text} />
                <div class="rowActions" data-slop-export="hide">
                  <button aria-label="Move ingredient up" disabled={index === 0} onclick={() => moveRow("ingredients", item.$id, -1)}><ArrowUp size={10} /></button>
                  <button aria-label="Move ingredient down" disabled={index === doc.current.ingredients.length - 1} onclick={() => moveRow("ingredients", item.$id, 1)}><ArrowDown size={10} /></button>
                  <button aria-label="Remove ingredient" onclick={() => doc.fields.ingredients.remove(item.$id)}><Trash2 size={10} /></button>
                </div>
              </li>
            {:else}
              <li class="empty">Add what you need to gather.</li>
            {/each}
          </ul>
        </section>

        <section class="section" aria-labelledby="method-heading">
          <div class="sectionTitle"><div><h2 id="method-heading">Method</h2></div><button data-slop-export="hide" aria-label="Add step" onclick={addStep}><Plus size={12} /></button></div>
          <ol class="list">
            {#each doc.current.steps as step, index (step.$id)}
              <li class="stepRow">
                <span class="stepNumber">{String(index + 1).padStart(2, "0")}</span>
                <div class="stepCopy">
                  <input class="stepTitle" aria-label="Step {index + 1} title" use:bindText={doc.at(step).title} />
                  <textarea use:grow={step.text} aria-label="Step {index + 1} instructions" use:bindText={doc.at(step).text}></textarea>
                </div>
                <label class="stepTime" title="Optional timer"><input aria-label="Step {index + 1} timer in minutes" type="number" min="0" placeholder="–" use:bindOptionalNumber={{ value: step.minutes, min: 0, max: 9999, set: (value) => doc.at(step).minutes.set(value), clear: () => doc.at(step).minutes.clear() }} /><span>min</span></label>
                <div class="rowActions" data-slop-export="hide">
                  <button aria-label="Move step up" disabled={index === 0} onclick={() => moveRow("steps", step.$id, -1)}><ArrowUp size={10} /></button>
                  <button aria-label="Move step down" disabled={index === doc.current.steps.length - 1} onclick={() => moveRow("steps", step.$id, 1)}><ArrowDown size={10} /></button>
                  <button aria-label="Remove step" onclick={() => removeStep(step.$id)}><Trash2 size={10} /></button>
                </div>
              </li>
            {:else}
              <li class="empty">Add the first step, then start cooking.</li>
            {/each}
          </ol>
        </section>
      </div>
    </article>

    {#if photoError}
      <p class="error" role="alert">
        That photo couldn’t be added. Try another image.
        <button data-slop-export="hide" onclick={() => picker?.click()}>Choose photo</button>
      </p>
    {/if}
  </main>

  {#snippet exportView()}
    <article class="card" aria-label="Exported recipe for {doc.current.title}">
      <header class="hero" style:grid-template-columns={photoUrl ? undefined : "1fr"}>
        <div class="intro">
          <div class="utilityLine">
            <span class="difficultyTrigger">{doc.current.difficulty}</span>
          </div>
          <h1 class="titleText">{doc.current.title.trim() || "Untitled recipe"}</h1>
          {#if doc.current.description.trim()}<p class="descriptionText">{doc.current.description}</p>{/if}
          <div class="stats">
            <div><span>Serves</span><strong>{doc.current.servings ?? "—"}</strong></div>
            <div><span>Prep</span><span class="number"><strong>{doc.current.prepMinutes ?? "—"}</strong><small>min</small></span></div>
            <div><span>Cook</span><span class="number"><strong>{doc.current.cookMinutes ?? "—"}</strong><small>min</small></span></div>
          </div>
        </div>
        {#if photoUrl}<figure class="photoWell" data-photo="true">
          <img src={photoUrl} alt="" />
        </figure>{/if}
      </header>

      <div class="body">
        <section class="section" aria-labelledby="export-ingredients">
          <div class="sectionTitle"><div><h2 id="export-ingredients">Ingredients</h2></div></div>
          <ul class="list">
            {#each doc.current.ingredients as item (item.$id)}
              <li class="ingredientRow" data-checked={item.checked}>
                <span data-checkbox-root data-state={item.checked ? "checked" : "unchecked"}></span>
                <span class="itemText">{item.text.trim() || "Untitled ingredient"}</span>
              </li>
            {:else}
              <li class="empty">No ingredients yet.</li>
            {/each}
          </ul>
        </section>
        <section class="section" aria-labelledby="export-method">
          <div class="sectionTitle"><div><h2 id="export-method">Method</h2></div></div>
          <ol class="list">
            {#each doc.current.steps as step, index (step.$id)}
              <li class="stepRow">
                <span class="stepNumber">{String(index + 1).padStart(2, "0")}</span>
                <div class="stepCopy">
                  <h3 class="stepTitleText">{step.title.trim() || `Step ${index + 1}`}</h3>
                  {#if step.text.trim()}<p class="stepBodyText">{step.text}</p>{/if}
                </div>
                {#if step.minutes}<span class="stepTime">{step.minutes} min</span>{/if}
              </li>
            {:else}
              <li class="empty">No steps yet.</li>
            {/each}
          </ol>
        </section>
      </div>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">{@render cookbook()}</div>
  {/snippet}
</Slop>
