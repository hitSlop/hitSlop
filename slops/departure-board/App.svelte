<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { getDocument } from "hitslop/svelte";
  import { onDestroy, onMount } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Check from "@lucide/svelte/icons/check";
  import Volume from "@lucide/svelte/icons/volume-2";
  import VolumeOff from "@lucide/svelte/icons/volume-x";
  import schema from "./schema";
  import { addStop, boardView, cells, cycleStatus, editStop, removeStop, setStatus, setTitle, statusLabel, statuses, type Status } from "./board";
  import { flap } from "./flap";
  import { clack, disableSound, enableSound } from "./clack";

  const doc = getDocument(schema);

  type Row = (typeof doc.current.rows)[number];

  // The clock, the edit sheet and sound are presentation only; none of it is saved.
  let editing = $state(false);
  let sound = $state(false);
  let now = $state(new Date());
  const clock = $derived(`${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`);
  const view = $derived(boardView(doc.current));
  const rows = $derived(doc.current.rows);
  const instant = $derived(prefersReducedMotion.current);

  onMount(() => {
    const tick = setInterval(() => (now = new Date()), 5000);
    return () => clearInterval(tick);
  });
  onDestroy(disableSound);

  function toggleSound() {
    sound = !sound;
    if (sound) enableSound(); else disableSound();
  }
  const cycle = (row: Row) => doc.change(tx => cycleStatus(tx, doc.current, row));
  const add = () => doc.change(tx => addStop(tx, doc.current, clock, "NEW STOP"));
</script>

<main class="board-shell" aria-label="Departure board">
  <section class="board" aria-label={doc.current.title || "Departures"}>
    <header class="head">
      <h1>{view.heading}</h1>
      <div class="clock" aria-label={`Now ${clock}`}>
        {#each cells(clock, 5) as char, i (i)}<span class="flap" aria-hidden="true" use:flap={{ char, delay: 0, instant, onflip: sound ? clack : undefined }}></span>{/each}
      </div>
    </header>
    <p class="labels" aria-hidden="true"><span>Time</span><span>Destination</span><span>Status</span></p>

    <ul class="rows">
      {#each view.rows as { row, label, groups }, r (row.$id)}
        <li>
          <button class="row" data-status={row.status} onclick={() => cycle(row)} aria-label={`${label}. Tap to change status.`}>
            {#each groups as group (group.kind)}
              <span class="group {group.kind}" aria-hidden="true">
                {#each group.cells as char, i (i)}
                  <span class="flap" use:flap={{ char, delay: 0.15 + r * 0.14 + i * 0.03, instant, onflip: sound ? clack : undefined }}></span>
                {/each}
              </span>
            {/each}
          </button>
        </li>
      {:else}
        <li class="empty"><p>No departures.</p></li>
      {/each}
    </ul>
  </section>

  <div class="bar">
    <Button.Root class="pill" aria-pressed={editing} onclick={() => (editing = !editing)}>
      {#if editing}<Check size={16} />Done{:else}<Pencil size={16} />Edit stops{/if}
    </Button.Root>
    <Button.Root class="pill" aria-pressed={sound} onclick={toggleSound}>
      {#if sound}<Volume size={16} />Sound on{:else}<VolumeOff size={16} />Sound off{/if}
    </Button.Root>
    <span class="hint">Tap a row to change its status</span>
  </div>

  {#if editing}
    <section class="sheet" aria-label="Edit stops">
      <label class="field">
        <span>Board title</span>
        <input value={doc.current.title} maxlength="24" onchange={(event) => doc.change(tx => setTitle(tx, event.currentTarget.value))} />
      </label>
      <ul class="edit-rows">
        {#each rows as row, r (row.$id)}
          <li>
            <input class="e-time" aria-label={`Stop ${r + 1} time`} value={row.time} maxlength="5" placeholder="12:00" onchange={(event) => { const time = event.currentTarget.value; doc.change(tx => editStop(tx, doc.current, row, { time })); }} />
            <input class="e-text" aria-label={`Stop ${r + 1} name`} value={row.text} maxlength="14" placeholder="Where to" onchange={(event) => { const text = event.currentTarget.value; doc.change(tx => editStop(tx, doc.current, row, { text })); }} />
            <select aria-label={`Stop ${r + 1} status`} value={row.status} onchange={(event) => { const status = event.currentTarget.value as Status; doc.change(tx => setStatus(tx, doc.current, row, status)); }}>
              {#each statuses as status}<option value={status}>{statusLabel[status]}</option>{/each}
            </select>
            <button class="x" aria-label={`Remove stop ${r + 1}`} onclick={() => doc.change(tx => removeStop(tx, doc.current, row))}><X size={16} /></button>
          </li>
        {/each}
      </ul>
      <Button.Root class="pill add" onclick={add} disabled={view.full}><Plus size={16} />Add a stop{view.full ? " (board is full)" : ""}</Button.Root>
    </section>
  {/if}
</main>
