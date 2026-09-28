<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { crossfade } from "svelte/transition";
  import { Checkbox, Dialog, Button, RadioGroup, Tooltip } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import schema, { type Task, type Zone } from "./schema";

  type QuadrantKey = Exclude<Zone, "inbox">;

  const QUADRANTS: { key: QuadrantKey; num: string; title: string; subtitle: string; tag: string; empty: string }[] = [
    { key: "q1", num: "I", title: "Do First", subtitle: "Urgent & Important", tag: "Crises & Deadlines", empty: "No fires on the blotter." },
    { key: "q2", num: "II", title: "Schedule", subtitle: "Not Urgent & Important", tag: "Focus & Leverage", empty: "Nothing to grow yet." },
    { key: "q3", num: "III", title: "Delegate", subtitle: "Urgent & Not Important", tag: "Interruptions", empty: "Nothing to hand off." },
    { key: "q4", num: "IV", title: "Don’t Do", subtitle: "Not Urgent & Not Important", tag: "Eliminate", empty: "Nothing to drop." },
  ];
  const ZONES: { key: Zone; num: string; label: string }[] = [
    ...QUADRANTS.map(q => ({ key: q.key as Zone, num: q.num, label: q.title })),
    { key: "inbox", num: "IN", label: "Holding pen" },
  ];

  function isQuadrant(value: string): value is QuadrantKey {
    return value === "q1" || value === "q2" || value === "q3" || value === "q4";
  }
  function isZone(value: string): value is Zone {
    return value === "inbox" || isQuadrant(value);
  }

  const doc = useDocument(schema);

  let inboxDraft = $state("");
  let inboxRef = $state<HTMLInputElement>();
  let captureOpen = $state(false);
  let captureText = $state("");
  let captureDest = $state<Zone>("inbox");

  const inbox = $derived(doc.current.tasks.filter((task) => task.zone === "inbox"));
  const totalActive = $derived(doc.current.tasks.filter((task) => !task.done).length);
  const q2Active = $derived(doc.current.tasks.filter((task) => task.zone === "q2" && !task.done).length);
  const q2Ratio = $derived(totalActive > 0 ? Math.round((q2Active / totalActive) * 100) : 0);
  const filled = $derived({
    q1: doc.current.tasks.some((task) => task.zone === "q1" && !task.done),
    q2: doc.current.tasks.some((task) => task.zone === "q2" && !task.done),
    q3: doc.current.tasks.some((task) => task.zone === "q3" && !task.done),
    q4: doc.current.tasks.some((task) => task.zone === "q4" && !task.done),
  });
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);
  const [send, receive] = crossfade({ duration: () => prefersReducedMotion.current ? 0 : 220 });

  function tasksIn(zone: Zone): Task[] {
    return doc.current.tasks.filter((task) => task.zone === zone);
  }
  function addInbox() {
    const text = inboxDraft.trim();
    if (!text) return;
    doc.fields.tasks.insert({ text, done: false, zone: "inbox" });
    inboxDraft = "";
    inboxRef?.focus();
  }
  function openCapture(dest: Zone = "inbox") {
    captureDest = dest;
    captureText = "";
    captureOpen = true;
  }
  function submitCapture() {
    const text = captureText.trim();
    if (!text) return;
    doc.fields.tasks.insert({ text, done: false, zone: captureDest });
    captureText = "";
    captureOpen = false;
  }
  function deleteTask(id: string) {
    doc.fields.tasks.remove(id);
  }
  function moveTask(task: Task, to: QuadrantKey) {
    if (task.zone === to) return;
    doc.at(task).zone.set(to);
  }
  function destinations(from: Zone) {
    return QUADRANTS.filter(q => q.key !== from);
  }
</script>

<Slop>
<Tooltip.Provider>
<main class="canvas" data-slop-selection="none" aria-label="Eisenhower matrix blotter">
  <article class="blotter">
    <header class="letterhead">
      <div class="titleGroup">
        <input class="title" aria-label="Blotter title" use:bindText={doc.fields.title} placeholder="Priority Desk Blotter" />
        <input class="date" aria-label="Blotter date" use:bindText={doc.fields.date} />
      </div>
      <Tooltip.Root>
        <Tooltip.Trigger class="leverage" aria-label="Q2 leverage {q2Ratio} percent">
          <span class="leverageRow">
            <span>Q2 Leverage</span>
            <span class="leverageValue">{q2Ratio}%</span>
          </span>
          <span class="meter" aria-hidden="true"><span class="meterFill" style:width="{q2Ratio}%"></span></span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content class="tooltip" sideOffset={6}>Aim for more than half of open work in Schedule (Q2).</Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
      <span class="activeCount">{totalActive} open</span>
    </header>

    <div class="axis" aria-hidden="true">
      <span>◀ Urgent</span>
      <span>Not urgent ▶</span>
    </div>

    <div class="matrix">
      {#each QUADRANTS as q (q.key)}
        <section class="quadrant" data-quad={q.key} aria-label="{q.title} tasks">
          <div class="quadHeader">
            <div class="quadBadge">
              <span class="roman" data-quad={q.key}>{q.num}</span>
              <div class="quadText">
                <h2 class="quadTitle">{q.title}</h2>
                <span class="quadSub">{q.subtitle}</span>
              </div>
            </div>
            <span class="quadTag">{q.tag}</span>
            <Button.Root class="quadAdd" data-slop-export="hide" aria-label="Place a task in {q.title}" onclick={() => openCapture(q.key)}>
              <Plus size={13} strokeWidth={2.2} />
            </Button.Root>
          </div>
          <ul class="list">
            {#each tasksIn(q.key) as task (task.$id)}
              <li
                class="row"
                data-done={task.done}
                in:receive={{ key: task.$id }}
                out:send={{ key: task.$id }}
                animate:flip={{ duration: flipMs }}
              >
                <Checkbox.Root checked={task.done} onCheckedChange={checked => { doc.at(task).done.set(checked === true); }} aria-label={`Mark ${task.text || "untitled task"} ${task.done ? "open" : "done"}`}>
                  {#snippet children({ checked })}{#if checked}<Check size={12} strokeWidth={3} />{/if}{/snippet}
                </Checkbox.Root>
                <input class="taskText" aria-label="{q.title} task" use:bindText={doc.at(task).text} />
                <div class="actions" data-slop-export="hide">
                  <div class="stamps">
                    {#each destinations(q.key) as target}
                      <button type="button" class="stamp" data-quad={target.key} title="Move to {target.title} ({target.num})" aria-label="Move to {target.title}" onclick={() => moveTask(task, target.key)}>{target.num}</button>
                    {/each}
                  </div>
                  <button type="button" class="remove" aria-label="Delete {task.text || "untitled task"}" onclick={() => deleteTask(task.$id)}><Trash2 size={13} /></button>
                </div>
              </li>
            {:else}
              <li class="empty">{q.empty}</li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>

    <footer class="tray">
      <div class="trayHead">
        <div class="trayTitle">
          <h3 class="trayHeading">Holding Pen</h3>
          <span class="trayHint">Capture first, then stamp into I–IV</span>
        </div>
        <span class="trayCount">{inbox.length} parked</span>
      </div>
      {#if inbox.length > 0}
        <ul class="trayList" aria-label="Holding pen">
          {#each inbox as item (item.$id)}
            <li
              class="inboxRow"
              in:receive={{ key: item.$id }}
              out:send={{ key: item.$id }}
              animate:flip={{ duration: flipMs }}
            >
              <input class="taskText" aria-label="Inbox task" use:bindText={doc.at(item).text} />
              <div class="dispatch" data-slop-export="hide">
                <span class="dispatchLabel">Stamp</span>
                {#each QUADRANTS as target}
                  <button type="button" class="stamp" data-quad={target.key} title="Send to {target.title} ({target.num})" aria-label="Send to {target.title}" onclick={() => moveTask(item, target.key)}>{target.num}</button>
                {/each}
                <button type="button" class="remove" aria-label="Remove {item.text || "untitled task"}" onclick={() => deleteTask(item.$id)}><Trash2 size={13} /></button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
      <form class="composer" data-slop-export="hide" onsubmit={event => { event.preventDefault(); addInbox(); }}>
        <input
          bind:this={inboxRef}
          class="composerInput"
          bind:value={inboxDraft}
          aria-label="Quick capture a new task"
          placeholder="Quick capture a new task…"
        />
        <Button.Root class="place" type="button" onclick={() => openCapture("inbox")}>Place</Button.Root>
        <Button.Root class="add" type="submit" aria-label="Add to holding pen" disabled={!inboxDraft.trim()}><Plus size={16} /></Button.Root>
      </form>
    </footer>
  </article>
</main>

<Dialog.Root bind:open={captureOpen}>
  <Dialog.Portal>
    <Dialog.Overlay class="overlay" data-slop-export="hide" />
    <Dialog.Content class="dialog" aria-labelledby="capture-title" data-slop-export="hide">
      <div class="dialogHead">
        <Dialog.Title id="capture-title">Place on the blotter</Dialog.Title>
        <Dialog.Close class="dialogClose" aria-label="Close capture"><X size={16} /></Dialog.Close>
      </div>
      <form class="dialogForm" onsubmit={event => { event.preventDefault(); submitCapture(); }}>
        <label class="dialogField">
          <span>Task</span>
          <input bind:value={captureText} aria-label="Task to place" placeholder="What needs a square?" required />
        </label>
        <div class="dialogField">
          <span>Square</span>
          <RadioGroup.Root class="destGroup" orientation="horizontal" value={captureDest} onValueChange={value => { if (isZone(value)) captureDest = value; }} aria-label="Destination square">
            {#each ZONES as zone (zone.key)}
              <RadioGroup.Item value={zone.key} class="dest" data-quad={zone.key} aria-label={zone.label}>{zone.num}</RadioGroup.Item>
            {/each}
          </RadioGroup.Root>
        </div>
        <div class="dialogActions">
          <Button.Root class="dialogCancel" type="button" onclick={() => { captureOpen = false; }}>Cancel</Button.Root>
          <Button.Root class="dialogSubmit" type="submit" disabled={!captureText.trim()}>Place</Button.Root>
        </div>
      </form>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
</Tooltip.Provider>

{#snippet exportView()}
  <article class="exportBlotter" aria-label="Exported Eisenhower matrix {doc.current.title}">
    <header class="letterhead">
      <div class="titleGroup">
        <h1 class="title">{doc.current.title.trim() || "Priority Desk Blotter"}</h1>
        <p class="date">{doc.current.date.trim() || "Undated"}</p>
      </div>
      <div class="leverage" aria-label="Q2 leverage {q2Ratio} percent">
        <span class="leverageRow">
          <span>Q2 Leverage</span>
          <span class="leverageValue">{q2Ratio}%</span>
        </span>
        <span class="meter" aria-hidden="true"><span class="meterFill" style:width="{q2Ratio}%"></span></span>
      </div>
      <span class="activeCount">{totalActive} open</span>
    </header>

    <div class="axis" aria-hidden="true">
      <span>◀ Urgent</span>
      <span>Not urgent ▶</span>
    </div>

    <div class="exportMatrix">
      {#each QUADRANTS as q (q.key)}
        <section class="exportQuadrant" data-quad={q.key} aria-label="{q.title} tasks">
          <div class="quadHeader">
            <div class="quadBadge">
              <span class="roman" data-quad={q.key}>{q.num}</span>
              <div class="quadText">
                <h2 class="quadTitle">{q.title}</h2>
                <span class="quadSub">{q.subtitle}</span>
              </div>
            </div>
            <span class="quadTag">{q.tag}</span>
          </div>
          <ul class="exportList">
            {#each tasksIn(q.key) as task (task.$id)}
              <li class="row" data-done={task.done}>
                <span data-checkbox-root data-state={task.done ? "checked" : "unchecked"}>{#if task.done}<Check size={10} strokeWidth={3} />{/if}</span>
                <span class="exportText">{task.text.trim() || "Untitled task"}</span>
              </li>
            {:else}
              <li class="empty">{q.empty}</li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>

    <footer class="tray">
      <div class="trayHead">
        <div class="trayTitle">
          <h3 class="trayHeading">Holding Pen</h3>
        </div>
        <span class="trayCount">{inbox.length} parked</span>
      </div>
      {#if inbox.length > 0}
        <ul class="trayList" aria-label="Holding pen">
          {#each inbox as item (item.$id)}
            <li class="inboxRow">
              <span class="exportText">{item.text.trim() || "Untitled task"}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="empty">Nothing waiting to be stamped.</p>
      {/if}
    </footer>
  </article>
{/snippet}

{#snippet icon()}
  <div class="iconSurface" aria-hidden="true">
    <article class="iconSheet">
      <div class="iconHead">
        <span class="iconTitle"></span>
        <span class="iconMeter"><span class="iconMeterFill" style:width="{Math.max(q2Ratio, 12)}%"></span></span>
      </div>
      <div class="iconGrid">
        <span class="iconQuad" data-quad="q1" data-filled={filled.q1}></span>
        <span class="iconQuad" data-quad="q2" data-filled={filled.q2}></span>
        <span class="iconQuad" data-quad="q3" data-filled={filled.q3}></span>
        <span class="iconQuad" data-quad="q4" data-filled={filled.q4}></span>
      </div>
    </article>
  </div>
{/snippet}
</Slop>
