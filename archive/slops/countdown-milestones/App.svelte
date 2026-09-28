<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { Checkbox, Dialog, Progress } from "bits-ui";
  import { onMount } from "svelte";
  import schema, { type Milestone } from "./schema";
  import { parseTarget, countdownState } from "./countdown";
  import CountdownFace from "./CountdownFace.svelte";
  import DatePicker from "./DatePicker.svelte";
  import { grow } from "./grow";

  const doc = useDocument(schema);
  let now = $state(Date.now());
  const view = $derived(countdownState(doc.current.targetDate, doc.current.targetTime, now));
  const completed = $derived(doc.current.milestones.filter((item) => item.done).length);
  let editing = $state(false);
  let editingSteps = $state(false);
  let title = $state("");
  let date = $state("");
  let time = $state("");
  let error = $state("");
  let newTitle = $state("");

  onMount(() => {
    const update = () => {
      now = Date.now();
    };
    const timer = setInterval(update, 1000);
    document.addEventListener("visibilitychange", update);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", update);
    };
  });

  function edit() {
    title = doc.current.title;
    date = doc.current.targetDate;
    time = doc.current.targetTime;
    error = "";
    editing = true;
  }

  function save() {
    const name = title.trim();
    if (!name) {
      error = "Give your event a name.";
      return;
    }
    if (!parseTarget(date, time)) {
      error = "Choose a valid date and local time.";
      return;
    }
    doc.change((tx) => {
      tx.fields.title.replace(name);
      tx.fields.targetDate.set(date);
      tx.fields.targetTime.set(time);
    });
    now = Date.now();
    editing = false;
  }

  function add() {
    const next = newTitle.trim();
    if (!next || doc.current.milestones.length >= 25) return;
    doc.fields.milestones.insert({ title: next, done: false });
    newTitle = "";
  }

  function move(id: string, step: number) {
    const items = doc.current.milestones;
    const index = items.findIndex((item) => item.$id === id);
    const next = index + step;
    if (index < 0 || next < 0 || next >= items.length) return;
    const neighbor = items[next]!;
    doc.fields.milestones.move(id, step < 0 ? { before: neighbor.$id } : { after: neighbor.$id });
  }
</script>

{#snippet milestoneList(items: readonly Milestone[], editable: boolean)}
  <ol class="list">
    {#each items as item, index (item.$id)}
      <li class="row">
        {#if editable}
          <Checkbox.Root
            class="check"
            checked={item.done}
            onCheckedChange={(checked) => doc.at(item).done.set(checked)}
            aria-label={`Complete ${item.title || "milestone"}`}
          >
            {#snippet children()}{item.done ? "✓" : ""}{/snippet}
          </Checkbox.Root>
          <textarea
            class="milestone"
            data-done={item.done}
            aria-label={`Milestone ${index + 1} title`}
            rows="1"
            use:bindText={doc.at(item).title}
            use:grow={item.title}
          ></textarea>
          {#if editingSteps}
            <div class="tools" data-slop-export="hide">
              <button class="smallButton" aria-label={`Move ${item.title} up`} disabled={index === 0} onclick={() => move(item.$id, -1)}>↑</button>
              <button class="smallButton" aria-label={`Move ${item.title} down`} disabled={index === items.length - 1} onclick={() => move(item.$id, 1)}>↓</button>
              <button class="smallButton" aria-label={`Delete ${item.title}`} onclick={() => doc.fields.milestones.remove(item.$id)}>×</button>
            </div>
          {/if}
        {:else}
          <span class="check" data-state={item.done ? "checked" : "unchecked"} aria-label={item.done ? "Complete" : "Incomplete"}>{item.done ? "✓" : ""}</span>
          <span class="milestone" data-done={item.done}>{item.title}</span>
        {/if}
      </li>
    {:else}
      {#if !editable}<li class="empty">No milestones yet.</li>{/if}
    {/each}
  </ol>
{/snippet}

<Slop>
  <main class="shell" aria-label="Countdown and milestones">
    <header class="header">
      <span class="eyebrow">A date to remember</span>
      <button class="button" data-slop-export="hide" onclick={edit}>Edit countdown</button>
    </header>
    <CountdownFace data={doc.current} state={view} onEdit={edit} />
    <section aria-label="Milestones">
      <div class="sectionHead">
        <h2 class="sectionTitle">Along the way</h2>
        <button class="smallButton" data-slop-export="hide" onclick={() => (editingSteps = !editingSteps)} aria-pressed={editingSteps}>
          {editingSteps ? "Done editing" : "Edit steps"}
        </button>
      </div>
      <div class="progressRow">
        <span>{completed} of {doc.current.milestones.length} complete</span>
        <Progress.Root class="track" value={completed} max={Math.max(1, doc.current.milestones.length)} aria-label="Milestone completion">
          <div class="fill" style:width={`${doc.current.milestones.length ? (completed / doc.current.milestones.length) * 100 : 0}%`}></div>
        </Progress.Root>
      </div>
      {@render milestoneList(doc.current.milestones, true)}
      {#if !doc.current.milestones.length}
        <p class="empty">Anything to do before the big day? Add your first step.</p>
      {/if}
      <form
        class="add"
        data-slop-export="hide"
        onsubmit={(event) => {
          event.preventDefault();
          add();
        }}
      >
        <input
          class="addInput"
          bind:value={newTitle}
          maxlength="120"
          placeholder="Add a milestone…"
          aria-label="New milestone"
          disabled={doc.current.milestones.length >= 25}
        />
        <button class="addButton" type="submit" disabled={!newTitle.trim() || doc.current.milestones.length >= 25}>+ Add</button>
      </form>
      {#if doc.current.milestones.length >= 25}
        <p class="empty">All 25 milestone spaces are filled.</p>
      {/if}
    </section>
  </main>

  <Dialog.Root bind:open={editing}>
    <Dialog.Portal>
      <Dialog.Overlay class="overlay" data-slop-export="hide" />
      <Dialog.Content class="dialog" data-slop-export="hide">
        <Dialog.Title class="dialogTitle">Something to look forward to.</Dialog.Title>
        <Dialog.Description class="description">Choose the date and time in your local timezone.</Dialog.Description>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            save();
          }}
        >
          <label class="field">
            Event name
            <input class="input" bind:value={title} maxlength="100" required />
          </label>
          <div class="field">
            <span>Target date</span>
            <DatePicker bind:value={date} />
          </div>
          <label class="field">
            Time
            <input class="input" type="time" bind:value={time} required />
          </label>
          {#if error}<p class="error" role="alert">{error}</p>{/if}
          <div class="actions">
            <Dialog.Close class="button" type="button">Cancel</Dialog.Close>
            <button class="save" type="submit">Save countdown</button>
          </div>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  {#snippet exportView()}
    {@const done = doc.current.milestones.filter((item) => item.done).length}
    <article class="exportShell">
      <header class="header">
        <span class="eyebrow">A date to remember</span>
      </header>
      <CountdownFace data={doc.current} state={view} />
      <div class="sectionHead">
        <h2 class="sectionTitle">Along the way</h2>
        <span class="eyebrow">{done}/{doc.current.milestones.length}</span>
      </div>
      {@render milestoneList(doc.current.milestones, false)}
    </article>
  {/snippet}

  {#snippet icon()}
    {@const value = view.kind === "today" ? "Today" : view.value.length > 4 ? "999+" : view.value}
    <svg class="icon" viewBox="0 0 512 512" fill="none" aria-hidden="true">
      <rect x="22" y="24" width="468" height="466" rx="70" fill="#b9dcff" />
      <rect x="63" y="102" width="386" height="338" rx="34" fill="#25212c25" />
      <rect x="63" y="88" width="386" height="338" rx="34" fill="#d7eaff" />
      <rect x="63" y="76" width="386" height="338" rx="34" fill="#edf6ff" stroke="#25212c30" stroke-width="2" />
      {#each [151, 337] as x (x)}
        <rect {x} y="46" width="24" height="63" rx="12" fill="#c84c41" />
        <rect {x} y="42" width="24" height="63" rx="12" fill="#f45d4f" />
      {/each}
      <text x="256" y="264" text-anchor="middle" fill="#25212c" font-family="Countdown Fredoka, sans-serif" font-weight="550" font-size={value.length > 3 ? 94 : 132} letter-spacing="-2">{value}</text>
      <rect x="129" y="296" width="254" height="47" rx="8" fill="#f2d94e" />
      <text x="256" y="327" text-anchor="middle" fill="#25212c" font-family="Countdown Fredoka, sans-serif" font-size="25">{view.kind === "today" ? "It’s today!" : view.unit}</text>
    </svg>
  {/snippet}
</Slop>
