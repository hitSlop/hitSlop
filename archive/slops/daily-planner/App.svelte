<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { Checkbox, Dialog, Select } from "bits-ui";
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import ArrowDownToLine from "@lucide/svelte/icons/arrow-down-to-line";
  import NotebookPen from "@lucide/svelte/icons/notebook-pen";
  import schema, { KINDS, type Block, type Kind } from "./schema";
  import DatePicker from "./DatePicker.svelte";
  import {
    DAY_START,
    DAY_END,
    HOURS,
    SNAP,
    today,
    localDate,
    formatDate,
    layout,
    offsetOf,
    rangeOf,
    toClock,
    toLabel,
    bookedMinutesOf,
    adjustRange,
    validRange,
    scrollForTime,
  } from "./schedule";

  const doc = useDocument(schema);
  let now = $state(new Date());
  let follow = $state(true);
  let viewport = $state<HTMLDivElement>();
  let field = $state<HTMLDivElement>();
  let alignedDate = "";
  const isToday = $derived(doc.current.date === localDate(now));
  const nowMinutes = $derived(now.getHours() * 60 + now.getMinutes());
  const booked = $derived(bookedMinutesOf(doc.current.blocks));
  let editOpen = $state(false);
  type Draft = { id: string; title: string; start: string; end: string; kind: Kind };
  let editing = $state<Draft>({ id: "", title: "", start: "09:00", end: "10:00", kind: "focus" });
  let isNew = $state(false);
  let formError = $state("");
  let notesOpen = $state(false);
  let notesDraft = $state("");
  let returnFocus: HTMLElement | null = null;

  function pauseFollow() {
    follow = false;
  }

  function focusTime(minutes: number) {
    if (!viewport || !field) return;
    const hour = field.offsetHeight / HOURS.length;
    viewport.scrollTop = scrollForTime(minutes, hour, viewport.clientHeight, viewport.scrollHeight);
  }

  $effect(() => {
    const date = doc.current.date;
    if (!viewport || !field || alignedDate === date) return;
    alignedDate = date;
    untrack(() => {
      follow = true;
      void tick().then(() => focusTime(date === localDate(now) ? nowMinutes : 480));
    });
  });

  $effect(() => {
    const clock = setInterval(() => {
      now = new Date();
      if (follow && isToday && !editOpen && !notesOpen && !gesture) focusTime(nowMinutes);
    }, 60_000);
    const refresh = () => {
      if (document.visibilityState === "visible") {
        now = new Date();
        if (follow && isToday) focusTime(nowMinutes);
      }
    };
    document.addEventListener("visibilitychange", refresh);
    return () => {
      clearInterval(clock);
      document.removeEventListener("visibilitychange", refresh);
    };
  });

  function goNow() {
    now = new Date();
    doc.fields.date.set(today());
    follow = true;
    void tick().then(() => focusTime(nowMinutes));
  }

  function openEditor(
    block?: Pick<Block, "$id" | "title" | "start" | "end" | "kind">,
    start = Math.min(Math.max(nowMinutes, DAY_START), DAY_END - 60),
    end = start + 60,
  ) {
    pauseFollow();
    returnFocus = document.activeElement as HTMLElement;
    isNew = !block;
    formError = "";
    editing = block
      ? { id: block.$id, title: block.title, start: block.start, end: block.end, kind: block.kind }
      : {
          id: "",
          title: "",
          start: toClock(Math.floor(start / SNAP) * SNAP),
          end: toClock(Math.min(Math.floor(end / SNAP) * SNAP, DAY_END)),
          kind: "focus",
        };
    editOpen = true;
  }

  function saveBlock(event: SubmitEvent) {
    event.preventDefault();
    if (!validRange(editing.start, editing.end)) {
      formError = "Choose at least 15 minutes between 6am and midnight. Use 00:00 for midnight.";
      return;
    }
    if (!editing.title.trim()) {
      formError = "Give this block a title.";
      return;
    }
    const title = editing.title.trim();
    if (isNew) {
      doc.fields.blocks.insert({ title, start: editing.start, end: editing.end, kind: editing.kind });
    } else {
      const block = doc.current.blocks.find((entry) => entry.$id === editing.id);
      if (block) {
        doc.change((tx) => {
          const handle = tx.at(block);
          handle.title.replace(title);
          handle.start.set(editing.start);
          handle.end.set(editing.end);
          handle.kind.set(editing.kind);
        });
      }
    }
    editOpen = false;
  }

  function deleteBlock() {
    if (!editing.id) return;
    doc.fields.blocks.remove(editing.id);
    editOpen = false;
  }

  function restoreFocus(event: Event) {
    event.preventDefault();
    (returnFocus?.isConnected ? returnFocus : document.querySelector<HTMLElement>("[data-add-block]"))?.focus({
      preventScroll: true,
    });
  }

  type Gesture = {
    id: string | null;
    mode: "move" | "start" | "end" | "draw";
    anchor: number;
    start: number;
    end: number;
    originY: number;
    moved: boolean;
  };
  let gesture = $state<Gesture | null>(null);
  let preview = $state<{ start: number; end: number } | null>(null);
  let pointerY = 0;
  let raf = 0;
  const placed = $derived(
    layout(
      doc.current.blocks.map((block) =>
        gesture?.id === block.$id && preview
          ? { ...block, start: toClock(preview.start), end: toClock(preview.end) }
          : block,
      ),
    ),
  );

  function minutesAt(y: number) {
    const rect = field?.getBoundingClientRect();
    return rect
      ? Math.max(
          DAY_START,
          Math.min(
            DAY_END,
            Math.round((DAY_START + ((y - rect.top) / rect.height) * (DAY_END - DAY_START)) / SNAP) * SNAP,
          ),
        )
      : DAY_START;
  }

  function startGesture(event: PointerEvent, block?: Block, mode: "move" | "start" | "end" = "move") {
    if (event.button !== 0) return;
    pauseFollow();
    const anchor = Math.min(minutesAt(event.clientY), DAY_END - SNAP);
    gesture = {
      id: block?.$id ?? null,
      mode: block ? mode : "draw",
      anchor,
      ...(block ? rangeOf(block) : { start: anchor, end: anchor + SNAP }),
      originY: event.clientY,
      moved: false,
    };
    preview = { start: gesture.start, end: gesture.end };
    pointerY = event.clientY;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
    event.stopPropagation();
    raf = requestAnimationFrame(autoScroll);
  }

  function updatePreview() {
    if (!gesture) return;
    const at = minutesAt(pointerY);
    preview = gesture.mode === "draw"
      ? { start: Math.min(gesture.anchor, at), end: Math.min(DAY_END, Math.max(gesture.anchor + SNAP, at)) }
      : adjustRange(gesture, gesture.mode, at - gesture.anchor);
  }

  function moveGesture(event: PointerEvent) {
    if (!gesture) return;
    pointerY = event.clientY;
    if (Math.abs(pointerY - gesture.originY) > 3) gesture.moved = true;
    if (gesture.moved) updatePreview();
  }

  function autoScroll() {
    if (!gesture || !viewport) return;
    const rect = viewport.getBoundingClientRect();
    const delta = pointerY < rect.top + 32 ? -6 : pointerY > rect.bottom - 32 ? 6 : 0;
    if (delta && gesture.moved) {
      viewport.scrollTop += delta;
      updatePreview();
    }
    raf = requestAnimationFrame(autoScroll);
  }

  function cancelGesture() {
    cancelAnimationFrame(raf);
    gesture = null;
    preview = null;
  }

  function endGesture() {
    if (!gesture || !preview) return;
    const g = gesture;
    const next = preview;
    cancelGesture();
    if (!g.id) {
      const start = g.moved ? next.start : Math.min(next.start, DAY_END - 60);
      const end = g.moved ? next.end : start + 60;
      openEditor(undefined, start, end);
      return;
    }
    const block = doc.current.blocks.find((entry) => entry.$id === g.id);
    if (!block) return;
    if (g.moved) {
      doc.change((tx) => {
        const handle = tx.at(block);
        handle.start.set(toClock(next.start));
        handle.end.set(toClock(next.end));
      });
    } else if (g.mode === "move") openEditor(block);
  }

  function nudge(event: KeyboardEvent, block: Block, mode: "move" | "start" | "end") {
    const delta = event.key === "ArrowUp" ? -SNAP : event.key === "ArrowDown" ? SNAP : 0;
    if (!delta) return;
    event.preventDefault();
    pauseFollow();
    const next = adjustRange(rangeOf(block), mode, delta);
    const live = doc.current.blocks.find((entry) => entry.$id === block.$id);
    if (!live) return;
    doc.change((tx) => {
      const handle = tx.at(live);
      handle.start.set(toClock(next.start));
      handle.end.set(toClock(next.end));
    });
  }

  onDestroy(() => cancelAnimationFrame(raf));
</script>

<svelte:window onkeydown={(event) => { if (event.key === "Escape") cancelGesture(); }} />
<Slop>
  <main class="canvas" data-slop-selection="none" aria-label="Daily planner">
    <article class="page">
      <header class="header">
        <div class="headerTop">
          <span class="eyebrow">DAILY / PLANNER</span>
          <button class="nowButton" onclick={goNow} aria-label="Go to now" data-following={follow && isToday}>
            <ArrowDownToLine size={13} /> Now
          </button>
        </div>
        <div class="headerMain">
          <h1 class="title">{formatDate(doc.current.date, { weekday: "long" })}</h1>
          <span class="dateNumber">{formatDate(doc.current.date, { day: "2-digit" })}</span>
        </div>
        <DatePicker value={doc.current.date} onChange={(value) => doc.fields.date.set(value)} />
      </header>
      <section class="priorities" aria-label="Top three priorities">
        <div class="sectionHead">
          <h2 class="heading">Make room for</h2>
          <span class="meta">{doc.current.priorities.filter((priority) => priority.done).length} / {doc.current.priorities.length}</span>
        </div>
        <ol class="priorityList">
          {#each doc.current.priorities as priority, index (priority.$id)}
            <li class="priority" data-done={priority.done}>
              <Checkbox.Root
                class="checkbox"
                checked={priority.done}
                onCheckedChange={(checked) => doc.at(priority).done.set(checked === true)}
                aria-label="Complete priority {index + 1}"
              >
                {#snippet children({ checked })}
                  {#if checked}<Check size={12} />{:else}{index + 1}{/if}
                {/snippet}
              </Checkbox.Root>
              <input
                class="priorityText"
                aria-label="Priority {index + 1}"
                placeholder="A little room for…"
                use:bindText={doc.at(priority).text}
                onfocus={pauseFollow}
              />
            </li>
          {/each}
        </ol>
      </section>
      <div class="scheduleHead">
        <div>
          <h2 class="heading">Your day</h2>
          <span class="meta">{Math.floor(booked / 60)}h {booked % 60 ? `${booked % 60}m ` : ""}planned</span>
        </div>
        <button class="addButton" data-add-block onclick={() => openEditor()}><Plus size={14} /> Add block</button>
      </div>
      <!-- The labeled scroll region is keyboard-focusable so arrow/page keys can scroll it. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="scroll"
        bind:this={viewport}
        onwheel={pauseFollow}
        ontouchstart={pauseFollow}
        onpointerdown={pauseFollow}
        onkeydown={(event) => {
          if (["ArrowDown", "ArrowUp", "PageDown", "PageUp", "Home", "End", " "].includes(event.key)) pauseFollow();
        }}
        tabindex="0"
        role="region"
        aria-label="Day timeline"
      >
        <div class="rail">
          <div class="gutter" aria-hidden="true">
            {#each HOURS as hour}<span class="hour">{toLabel(hour)}</span>{/each}
          </div>
          <div class="field" bind:this={field}>
            <button
              class="sheet"
              aria-label="Draw a time block"
              onpointerdown={(event) => startGesture(event)}
              onpointermove={moveGesture}
              onpointerup={endGesture}
              onpointercancel={cancelGesture}
              onkeydown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  openEditor();
                }
              }}
            ></button>
            {#if !placed.length}
              <p class="empty">A little structure.<br />A lot of possibility.<small>Drag to make time for something.</small></p>
            {/if}
            {#each placed as item (item.block.$id)}
              <article
                class="block"
                data-kind={item.block.kind}
                data-short={item.end - item.start < 60}
                data-tiny={item.end - item.start <= 15}
                data-active={gesture?.id === item.block.$id}
                style="--top:{offsetOf(item.start)};--span:{(item.end - item.start) / 60};--column:{item.column};--columns:{item.columns}"
              >
                <button
                  class="blockBody"
                  aria-label="Edit {item.block.title || 'Untitled block'}, {toLabel(item.start)} to {toLabel(item.end)}, {item.block.kind}"
                  title="{item.block.title} · {toLabel(item.start)}–{toLabel(item.end)} · {item.block.kind}"
                  onpointerdown={(event) => startGesture(event, item.block)}
                  onpointermove={moveGesture}
                  onpointerup={endGesture}
                  onpointercancel={cancelGesture}
                  onkeydown={(event) => nudge(event, item.block, "move")}
                  onclick={(event) => { if (event.detail === 0) openEditor(item.block); }}
                >
                  <strong class="blockTitle">{item.block.title || "Untitled block"}</strong>
                  <span class="blockDetail">{toLabel(item.start)}–{toLabel(item.end)} · {item.block.kind}</span>
                </button>
                {#each ["start", "end"] as edge}
                  <button
                    class="handle {edge === 'start' ? 'startHandle' : 'endHandle'}"
                    aria-label="Resize {edge} of {item.block.title}"
                    onpointerdown={(event) => startGesture(event, item.block, edge as "start" | "end")}
                    onpointermove={moveGesture}
                    onpointerup={endGesture}
                    onpointercancel={cancelGesture}
                    onkeydown={(event) => nudge(event, item.block, edge as "start" | "end")}
                  ></button>
                {/each}
              </article>
            {/each}
            {#if gesture?.mode === "draw" && preview}
              <div class="draft" style="--top:{offsetOf(preview.start)};--span:{(preview.end - preview.start) / 60}">
                {toLabel(preview.start)}–{toLabel(preview.end)}
              </div>
            {/if}
            {#if isToday && nowMinutes >= DAY_START && nowMinutes < DAY_END}
              <div class="nowLine" style="--top:{offsetOf(nowMinutes)}" aria-label="Current time {toLabel(nowMinutes)}">
                <span class="nowTag">{toLabel(nowMinutes)}</span>
              </div>
            {/if}
          </div>
        </div>
      </div>
      <button
        class="notesButton"
        onclick={() => {
          pauseFollow();
          returnFocus = document.activeElement as HTMLElement;
          notesDraft = doc.current.notes;
          notesOpen = true;
        }}
      >
        <NotebookPen size={17} />
        <span><b>Day notes</b><small>{doc.current.notes || "Something to remember…"}</small></span>
        <span aria-hidden="true">↗</span>
      </button>
    </article>
  </main>
  <Dialog.Root bind:open={editOpen}>
    <Dialog.Portal>
      <Dialog.Overlay class="overlay" />
      <Dialog.Content class="dialog" onCloseAutoFocus={restoreFocus}>
        <Dialog.Title class="dialogTitle">{isNew ? "Make some time" : "Edit your block"}</Dialog.Title>
        <Dialog.Description class="description">A place for what matters today.</Dialog.Description>
        <form class="form" onsubmit={saveBlock}>
          <label class="label">
            What’s happening?
            <input class="input" bind:value={editing.title} placeholder="A little focused work" required />
          </label>
          <div class="timeRow">
            <label class="label">From<input class="input" type="time" step="900" bind:value={editing.start} required /></label>
            <label class="label">Until<input class="input" type="time" step="900" bind:value={editing.end} required /></label>
          </div>
          <label class="label" for="block-kind">Kind</label>
          <Select.Root type="single" bind:value={editing.kind}>
            <Select.Trigger id="block-kind" class="input">{editing.kind} ▾</Select.Trigger>
            <Select.Portal>
              <Select.Content class="selectContent" sideOffset={5}>
                <Select.Viewport>
                  {#each KINDS as kind}
                    <Select.Item class="selectItem" value={kind} label={kind}>{kind}</Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
          {#if formError}<p role="alert" class="error">{formError}</p>{/if}
          <div class="actions">
            {#if !isNew}<button class="deleteButton" type="button" onclick={deleteBlock}>Delete</button>{/if}
            <Dialog.Close class="secondary" type="button">Cancel</Dialog.Close>
            <button class="primary" type="submit">Save block</button>
          </div>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
  <Dialog.Root bind:open={notesOpen}>
    <Dialog.Portal>
      <Dialog.Overlay class="overlay" />
      <Dialog.Content class="dialog" onCloseAutoFocus={restoreFocus}>
        <Dialog.Title class="dialogTitle">Day notes</Dialog.Title>
        <Dialog.Description class="description">Keep a thought. Clear a little headspace.</Dialog.Description>
        <form
          class="form"
          onsubmit={(event) => {
            event.preventDefault();
            doc.fields.notes.replace(notesDraft);
            notesOpen = false;
          }}
        >
          <textarea class="textarea" aria-label="Notes for the day" bind:value={notesDraft} placeholder="Something to remember…"></textarea>
          <div class="actions">
            <Dialog.Close type="button" class="secondary">Cancel</Dialog.Close>
            <button type="submit" class="primary">Save notes</button>
          </div>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  {#snippet exportView()}
    <article class="exportPage" aria-label="Exported daily planner">
      <header class="header">
        <span class="eyebrow">DAILY / PLANNER</span>
        <div class="headerMain">
          <h1 class="title">{formatDate(doc.current.date, { weekday: "long" })}</h1>
          <span class="dateNumber">{formatDate(doc.current.date, { day: "2-digit" })}</span>
        </div>
        <p>{formatDate(doc.current.date, { month: "long", day: "numeric", year: "numeric" })}</p>
      </header>
      <section class="priorities" style="max-height:none;overflow:visible">
        <h2 class="heading">Make room for</h2>
        <ol class="priorityList">
          {#each doc.current.priorities as priority, index (priority.$id)}
            <li class="priority">
              <span class="checkbox">{priority.done ? "✓" : index + 1}</span>
              <span style:text-decoration={priority.done ? "line-through" : "none"}>{priority.text || "—"}</span>
            </li>
          {/each}
        </ol>
      </section>
      <div class="scheduleHead">
        <h2 class="heading">Your day</h2>
        <span class="meta">{Math.floor(booked / 60)}h {booked % 60}m planned</span>
      </div>
      <div class="exportList">
        {#each layout(doc.current.blocks) as item (item.block.$id)}
          <section class="exportBlock" data-kind={item.block.kind}>
            <span class="meta">{toLabel(item.start)}–{toLabel(item.end)} · {item.block.kind}</span>
            <h3 class="exportTitle">{item.block.title || "Untitled block"}</h3>
          </section>
        {:else}
          <p>No time blocks yet.</p>
        {/each}
      </div>
      <section class="exportNotes">
        <h2 class="heading">Day notes</h2>
        {doc.current.notes || "Nothing noted."}
      </section>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <article class="iconPage">
        <div class="iconPlate">
          <span class="iconBlock iconBlockA"></span>
          <span class="iconBlock iconBlockB"></span>
          <span class="iconBlock iconBlockC"></span>
        </div>
        <span class="iconNow"></span>
      </article>
    </div>
  {/snippet}
</Slop>
