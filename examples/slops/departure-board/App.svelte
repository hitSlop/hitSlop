<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Check from "@lucide/svelte/icons/check";
  import Volume from "@lucide/svelte/icons/volume-2";
  import VolumeOff from "@lucide/svelte/icons/volume-x";
  import doc, { type Row } from "./schema";
  import { cells, clean, maxRows, nextStatus, statusLabel, statuses, widths } from "./board";
  import { flap } from "./flap";
  import { clack, disableSound, enableSound } from "./clack";

  let editing = $state(false);
  let sound = $state(false);
  let now = $state(new Date());
  const clock = $derived(`${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`);
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
  const cycle = (row: Row) => doc.at(row).status.set(nextStatus(row.status));
  const add = () => doc.fields.rows.insert({ time: clock, text: "NEW STOP", status: "on-time" });
</script>

<main class="board-shell" data-slop-selection="none" aria-label="Departure board">
  <section class="board" aria-label={doc.current.title || "Departures"}>
    <header class="head">
      <h1>{clean(doc.current.title, 24) || "DEPARTURES"}</h1>
      <div class="clock" aria-label={`Now ${clock}`}>
        {#each cells(clock, 5) as char, i (i)}<span class="flap" aria-hidden="true" use:flap={{ char, delay: 0, instant, onflip: sound ? clack : undefined }}></span>{/each}
      </div>
    </header>
    <p class="labels" aria-hidden="true"><span>Time</span><span>Destination</span><span>Status</span></p>

    <ul class="rows">
      {#each rows as row, r (row.$id)}
        <li>
          <button class="row" data-status={row.status} onclick={() => cycle(row)} aria-label={`${row.time} ${row.text}: ${statusLabel[row.status]}. Tap to change status.`}>
            {#each [["time", row.time], ["text", row.text], ["status", statusLabel[row.status]]] as [kind, text] (kind)}
              <span class="group {kind}" aria-hidden="true">
                {#each cells(text!, widths[kind as keyof typeof widths]) as char, i (i)}
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
        <input value={doc.current.title} maxlength="24" onchange={(event) => doc.fields.title.set(clean(event.currentTarget.value, 24))} />
      </label>
      <ul class="edit-rows">
        {#each rows as row, r (row.$id)}
          {@const item = doc.at(row)}
          <li>
            <input class="e-time" aria-label={`Stop ${r + 1} time`} value={row.time} maxlength="5" placeholder="12:00" onchange={(event) => item.time.set(clean(event.currentTarget.value, 5))} />
            <input class="e-text" aria-label={`Stop ${r + 1} name`} value={row.text} maxlength="14" placeholder="Where to" onchange={(event) => item.text.set(clean(event.currentTarget.value, 14))} />
            <select aria-label={`Stop ${r + 1} status`} value={row.status} onchange={(event) => item.status.set(event.currentTarget.value as Row["status"])}>
              {#each statuses as status}<option value={status}>{statusLabel[status]}</option>{/each}
            </select>
            <button class="x" aria-label={`Remove stop ${r + 1}`} onclick={() => doc.fields.rows.remove(row.$id)}><X size={16} /></button>
          </li>
        {/each}
      </ul>
      <Button.Root class="pill add" onclick={add} disabled={rows.length >= maxRows}><Plus size={16} />Add a stop{rows.length >= maxRows ? " (board is full)" : ""}</Button.Root>
    </section>
  {/if}
</main>
