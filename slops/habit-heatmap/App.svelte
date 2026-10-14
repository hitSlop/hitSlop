<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { onMount, tick } from "svelte";
  import { AlertDialog, Checkbox, Dialog, RadioGroup, Tabs } from "bits-ui";
    import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Pencil from "@lucide/svelte/icons/pencil";
  import X from "@lucide/svelte/icons/x";
  import { getDocument } from "hitslop/svelte";
  import type { Value } from "hitslop";
  import schema, { colors } from "./schema";
  import { addHabit, dayKey, editHabit, heatmapView, labelDay, removeHabit as dropHabit, setCheckin, shortDay, weekdays, type HabitColor } from "./calendar";

  const doc = getDocument(schema);
  type Habit = Value<typeof schema.descriptor>["habits"][number];

  // View state: Export cannot see it and shows every habit.
  let today = $state(dayKey(new Date()));
  let highlightedDay = $state<string | null>(null);
  let notice = $state("");

  let selectedID = $state<string | null>(null);

  const view = $derived(heatmapView(doc.current, today));
  const { days, labels } = $derived(view);
  const active = $derived(doc.current.habits.find(habit => habit.$id === selectedID) ?? doc.current.habits[0]);
  const activeStats = $derived(view.habits.find(entry => entry.habit.$id === active?.$id));

  let dialogOpen = $state(false);
  let removeOpen = $state(false);
  let removeError = $state("");
  const editingHabit = $derived(doc.current.habits.find(habit => habit.$id === editingID));
  let editingID = $state<string | null>(null);
  let draftName = $state("");
  let draftColor = $state<HabitColor>("mint");
  let formError = $state("");

  onMount(() => {
    const update = () => { today = dayKey(new Date()); };
    const timer = setInterval(update, 30_000);
    window.addEventListener("focus", update);
    return () => { clearInterval(timer); window.removeEventListener("focus", update); };
  });
  function begin(habit?: Habit) {
    editingID = habit?.$id ?? null;
    draftName = habit?.name ?? "";
    draftColor = habit?.color ?? "mint";
    formError = "";
    dialogOpen = true;
  }
  async function save() {
    const name = draftName.trim();
    if (!name) { formError = "Give this habit a name."; return; }
    try {
      // The id is available inside the collector; the change resolves once accepted.
      const editing = editingID;
      const id = await doc.change(tx => {
        if (editing !== null) {
          editHabit(tx, editing, name, draftColor);
          return editing;
        }
        return addHabit(tx, name, draftColor).id;
      });
      selectedID = id;
      dialogOpen = false;
      notice = editing ? "Habit updated." : "Habit added. Choose a day to begin.";
    } catch (error) { formError = error instanceof Error ? error.message : "Could not update this habit."; }
  }
  async function restoreEditorFocus(event: Event) {
    event.preventDefault();
    await tick();
    const target = document.querySelector<HTMLElement>('[data-tabs-trigger][data-state="active"]')
      ?? document.querySelector<HTMLElement>('.habit-empty button')
      ?? document.querySelector<HTMLElement>('.habit-add');
    target?.focus();
  }
  async function removeHabit() {
    if (!editingID) return;
    const habits = doc.current.habits;
    const index = habits.findIndex(habit => habit.$id === editingID);
    if (index < 0) return;
    const next = habits[index + 1] ?? habits[index - 1];
    try {
      const id = editingID;
      await doc.change(tx => dropHabit(tx, id));
      selectedID = next?.$id ?? null;
      highlightedDay = null;
      removeOpen = false;
      dialogOpen = false;
      notice = "Habit removed.";
    } catch (error) { removeError = error instanceof Error ? error.message : "Could not remove this habit."; }
  }
  function toggle(habit: Habit, day: string) {
    if (day > today) return;
    const done = Boolean(habit.checkins[day]);
    doc.change(tx => setCheckin(tx, habit.$id, day, !done, today)).then(
      () => (notice = `${habit.name}: ${shortDay(day)} ${done ? "cleared" : "complete"}.`),
    );
  }
</script>

{#snippet calendar(habit: Habit, editable: boolean)}
  <div class="habit-calendar" data-tone={habit.color}>
    <div class="habit-week-labels" aria-hidden="true">
      {#each labels as label}<span>{label}</span>{/each}
    </div>
    <div class="habit-weekdays" aria-hidden="true">{#each weekdays as day}<span>{day}</span>{/each}</div>
    <div class="habit-grid" role="group" aria-label={`${habit.name}: daily check-ins`}>
      {#each days as day}
        {#if editable}
          <Checkbox.Root class="habit-cell" checked={Boolean(habit.checkins[day])} disabled={day > today}
            onCheckedChange={() => toggle(habit, day)} aria-label={labelDay(day)}
            title={`${labelDay(day)}${day > today ? " · not yet" : habit.checkins[day] ? " · complete" : " · not complete"}`}
            data-day={day} data-today={day === today ? "" : undefined}
            onpointerenter={() => highlightedDay = day} onpointerleave={() => highlightedDay = null}
            onfocus={() => highlightedDay = day} onblur={() => highlightedDay = null}>
            {#if habit.checkins[day]}<Check size={14} strokeWidth={2.5} />{/if}
          </Checkbox.Root>
        {:else}
          <span class="habit-cell" data-state={habit.checkins[day] ? "checked" : "unchecked"}
            data-future={day > today ? "" : undefined} aria-label={`${labelDay(day)}: ${habit.checkins[day] ? "complete" : "not complete"}`}>
            {#if habit.checkins[day]}<Check size={14} strokeWidth={2.5} />{/if}
          </span>
        {/if}
      {/each}
    </div>
  </div>
{/snippet}


  <main class="habit-shell">
    <header class="habit-header">
      <div><p class="habit-eyebrow">Daily practice / 12 weeks</p><h1>Keep the thread</h1></div>
      <div class="habit-score" aria-label={`${activeStats?.streak ?? 0} day streak`}>
        <strong>{String(activeStats?.streak ?? 0).padStart(2, "0")}</strong><span>day<br />streak</span>
      </div>
    </header>
    <section class="habit-panel" aria-label="Habit record">
      <Tabs.Root activationMode="manual" value={active?.$id ?? ""} onValueChange={value => { selectedID = value; highlightedDay = null; }}>
        <div class="habit-tab-bar">
          <Tabs.List class="habit-tabs" aria-label="Habits">
            {#each doc.current.habits as habit (habit.$id)}
              <Tabs.Trigger value={habit.$id} data-tone={habit.color} title={habit.name}><i></i><span>{habit.name || "Untitled habit"}</span></Tabs.Trigger>
            {/each}
          </Tabs.List>
          <button class="habit-add" aria-label="Add habit" onclick={() => begin()}><Plus size={20} /></button>
        </div>
        {#if active}
          <Tabs.Content value={active.$id} class="habit-content">
            <div class="habit-detail">
              <h2>{active.name || "Untitled habit"}</h2>
              <button class="habit-edit" onclick={() => begin(active)} aria-label="Edit habit"><Pencil size={14} /> Edit</button>
              <p><strong>{activeStats?.completed ?? 0}</strong> check-ins</p>
            </div>
            {@render calendar(active, true)}
            <div class="habit-caption">
              <span>{shortDay(days[0]!)} — {shortDay(today)}</span>
              <span>{highlightedDay ? shortDay(highlightedDay) : "One square, one day."}</span>
            </div>
          </Tabs.Content>
        {:else}
          <div class="habit-empty"><h2>A small place to begin.</h2><p>Add something you would like to do a little more often.</p><button onclick={() => begin()}>Add your first habit</button></div>
        {/if}
      </Tabs.Root>
    </section>
    <footer class="habit-footer">Small things, repeated.</footer>
    <p class="habit-sr-only" aria-live="polite">{notice}</p>
  </main>
  <Dialog.Root bind:open={dialogOpen}>
    <Dialog.Portal>
      <Dialog.Overlay class="habit-overlay" />
      <Dialog.Content onCloseAutoFocus={restoreEditorFocus} class="habit-dialog">
        <form onsubmit={event => { event.preventDefault(); save(); }}>
          <Dialog.Title>{editingID ? "Keep it yours" : "Start a small habit"}</Dialog.Title>
          <Dialog.Description class="habit-sr-only">Choose a name and color for this habit.</Dialog.Description>
          <label class="habit-name-label" for="habit-name">Habit name</label>
          <input id="habit-name" bind:value={draftName} maxlength="80" placeholder="Read ten pages" aria-invalid={Boolean(formError)} aria-describedby={formError ? "habit-form-error" : undefined} />
          <fieldset class="habit-color-field"><legend>Color</legend>
            <RadioGroup.Root class="habit-colors" value={draftColor} onValueChange={value => draftColor = value as HabitColor} aria-label="Habit color">
              {#each colors as color}<RadioGroup.Item value={color} data-tone={color}><i></i><span>{color}</span></RadioGroup.Item>{/each}
            </RadioGroup.Root>
          </fieldset>
          {#if formError}<p id="habit-form-error" class="habit-form-error" role="alert">{formError}</p>{/if}
          <div class="habit-actions"><Dialog.Close type="button">Cancel</Dialog.Close><button type="submit">{editingID ? "Save changes" : "Add habit"}</button></div>
        </form>
        {#if editingID}
          <div class="habit-remove-row"><button class="habit-remove" onclick={() => { removeError = ""; removeOpen = true; }}>Remove habit</button></div>
        {/if}
        <AlertDialog.Root bind:open={removeOpen}>
          <AlertDialog.Portal>
            <AlertDialog.Overlay class="habit-overlay habit-remove-overlay" />
            <AlertDialog.Content class="habit-dialog habit-confirm"
              onCloseAutoFocus={event => { event.preventDefault(); if (dialogOpen) document.querySelector<HTMLElement>('.habit-remove')?.focus(); }}>
              <AlertDialog.Title>Remove {editingHabit?.name || "this habit"}?</AlertDialog.Title>
              <AlertDialog.Description>This removes the habit and all its check-in history.</AlertDialog.Description>
              {#if removeError}<p class="habit-form-error" role="alert">{removeError}</p>{/if}
              <div class="habit-actions"><AlertDialog.Cancel>Keep habit</AlertDialog.Cancel><button class="habit-remove-confirm" onclick={removeHabit}>Remove habit</button></div>
            </AlertDialog.Content>
          </AlertDialog.Portal>
        </AlertDialog.Root>
        <Dialog.Close class="habit-dialog-close" aria-label="Close"><X size={18} /></Dialog.Close>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
