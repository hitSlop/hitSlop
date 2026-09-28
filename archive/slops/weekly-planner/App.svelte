<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { Dialog, Select, Checkbox } from "bits-ui";
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import schema, { type Task, type TaskColor } from "./schema";
  import {
    HOUR,
    COLORS,
    minutes,
    clock,
    label,
    duration,
    snap,
    move,
    resize,
    layout,
    tasksByTime,
  } from "./schedule";

  const doc = useDocument(schema);
  let scroller = $state<HTMLDivElement>();
  let board = $state<HTMLDivElement>();

  onMount(() => {
    void tick().then(() =>
      requestAnimationFrame(() => {
        const lane = board?.querySelector<HTMLElement>("[data-lane]");
        if (lane && scroller) scroller.scrollTop = lane.offsetTop + 8 * HOUR - 57;
      }),
    );
  });

  const allTasks = $derived(doc.current.days.flatMap((day) => day.tasks));
  const doneCount = $derived(allTasks.filter((task) => task.done).length);
  let dialogOpen = $state(false);
  type Editor = {
    taskId: string | null;
    day: string;
    scheduled: boolean;
    task: {
      title: string;
      time: string;
      done: boolean;
      durationMinutes?: number;
      color: TaskColor;
    };
  };
  let editor = $state<Editor | null>(null);
  let message = $state("");
  let formError = $state("");
  const dayItems = $derived(doc.current.days.map((day) => ({ value: day.name, label: day.name })));
  const colorItems = COLORS.map((value) => ({
    value,
    label: value[0].toUpperCase() + value.slice(1),
  }));

  function isColor(value: string): value is TaskColor {
    return COLORS.some((color) => color === value);
  }

  function boundedDuration(value: number | undefined): number | undefined {
    return typeof value === "number" && Number.isInteger(value) && value >= 15 && value <= 1440
      ? value
      : undefined;
  }

  function edit(index: number, task?: Task, start?: number, span = 60) {
    const day = doc.current.days[index];
    if (!day) return;
    editor = {
      taskId: task?.$id ?? null,
      day: day.name,
      scheduled: task ? minutes(task.time) !== null : start !== undefined,
      task: task
        ? {
            title: task.title,
            time: task.time,
            done: task.done,
            durationMinutes: task.durationMinutes,
            color: task.color,
          }
        : {
            title: "",
            time: start === undefined ? "" : clock(start),
            done: false,
            durationMinutes: span,
            color: "sky",
          },
    };
    formError = "";
    dialogOpen = true;
  }

  function save() {
    const draft = editor;
    if (!draft) return;
    if (!draft.task.title.trim()) {
      formError = "Give this block a title.";
      return;
    }
    const start = minutes(draft.task.time);
    const durationMinutes = boundedDuration(draft.task.durationMinutes);
    if (draft.scheduled && (start === null || durationMinutes === undefined || start + durationMinutes > 1440)) {
      formError = "Choose a valid start and a duration that ends by midnight.";
      return;
    }
    const time = draft.scheduled ? draft.task.time : "";
    const target = doc.current.days.find((day) => day.name === draft.day);
    if (!target) return;
    const source = draft.taskId
      ? doc.current.days.find((day) => day.tasks.some((task) => task.$id === draft.taskId))
      : undefined;
    const existing = source?.tasks.find((task) => task.$id === draft.taskId);
    const fields = {
      title: draft.task.title,
      time,
      done: draft.task.done,
      color: draft.task.color,
      ...(durationMinutes === undefined ? {} : { durationMinutes }),
    };
    if (existing && source && source.$id === target.$id) {
      doc.change((tx) => {
        const handle = tx.at(existing);
        handle.title.replace(fields.title);
        handle.time.set(fields.time);
        handle.done.set(fields.done);
        handle.color.set(fields.color);
        if (durationMinutes === undefined) {
          if (existing.durationMinutes !== undefined) handle.durationMinutes.clear();
        } else handle.durationMinutes.set(durationMinutes);
      });
    } else {
      doc.change((tx) => {
        if (existing && source) tx.at(source).tasks.remove(existing.$id);
        tx.at(target).tasks.insert(fields);
      });
    }
    dialogOpen = false;
  }

  function remove() {
    const draft = editor;
    if (!draft?.taskId) return;
    const source = doc.current.days.find((day) => day.tasks.some((task) => task.$id === draft.taskId));
    if (source) doc.at(source).tasks.remove(draft.taskId);
    dialogOpen = false;
  }

  type Gesture = {
    source: number;
    id: string | null;
    mode: "move" | "start" | "end" | "draw";
    origin: number;
    start: number;
    end: number;
    x: number;
    y: number;
    moved: boolean;
    day: number;
    nextStart: number | null;
    nextEnd: number;
    anchor: number;
  };
  let gesture = $state<Gesture | null>(null);
  let pointer = { x: 0, y: 0 };
  let raf = 0;

  function hit(x: number, y: number) {
    const lanes = board?.querySelectorAll<HTMLElement>("[data-lane]");
    if (!lanes?.length) return { day: 0, at: 0, anytime: false };
    const first = lanes[0].getBoundingClientRect();
    const day = Math.max(0, Math.min(lanes.length - 1, Math.floor((x - first.left) / first.width)));
    return {
      day,
      at: Math.max(0, Math.min(1440, snap(((y - first.top) / HOUR) * 60))),
      anytime: y < first.top,
    };
  }

  function begin(event: PointerEvent, day: number, task: Task | null, mode: Gesture["mode"]) {
    if (event.button !== 0 || !board) return;
    event.stopPropagation();
    const at = hit(event.clientX, event.clientY).at;
    const start = Math.min(task ? (minutes(task.time) ?? at) : at, 1425);
    const end = Math.min(1440, start + (task ? duration(task) : 15));
    gesture = {
      source: day,
      id: task?.$id ?? null,
      mode,
      origin: at,
      start,
      end,
      x: event.clientX,
      y: event.clientY,
      moved: false,
      day,
      nextStart: task && minutes(task.time) === null ? null : start,
      nextEnd: end,
      anchor: start,
    };
    pointer = { x: event.clientX, y: event.clientY };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    if (mode !== "draw") event.preventDefault();
  }

  function updatePosition() {
    if (!gesture) return;
    const g = gesture;
    const point = hit(pointer.x, pointer.y);
    if (g.mode === "move") {
      g.day = point.day;
      g.nextStart = point.anytime ? null : move(g.start + point.at - g.origin, g.end - g.start);
      g.nextEnd = (g.nextStart ?? 0) + g.end - g.start;
    } else if (g.mode === "draw") {
      g.nextStart = Math.min(g.anchor, Math.min(point.at, 1425));
      g.nextEnd = Math.max(g.anchor + 15, point.at);
    } else {
      const next = resize(
        g.start,
        g.end,
        g.start + point.at - g.origin + (g.mode === "end" ? g.end - g.start : 0),
        g.mode,
      );
      g.nextStart = next.start;
      g.nextEnd = next.end;
    }
    message = `${doc.current.days[g.day]?.name}: ${g.nextStart === null ? "Anytime" : label(g.nextStart) + " – " + label(g.nextEnd)}`;
  }

  function autoScroll() {
    if (!gesture?.moved || !scroller) {
      raf = 0;
      return;
    }
    const rect = scroller.getBoundingClientRect();
    const speed = (value: number, min: number, max: number) =>
      value < min + 36 ? -10 : value > max - 36 ? 10 : 0;
    scroller.scrollBy(speed(pointer.x, rect.left + 48, rect.right), speed(pointer.y, rect.top + 58, rect.bottom));
    updatePosition();
    raf = requestAnimationFrame(autoScroll);
  }

  function pointerMove(event: PointerEvent) {
    if (!gesture) return;
    pointer = { x: event.clientX, y: event.clientY };
    if (Math.hypot(event.clientX - gesture.x, event.clientY - gesture.y) > 5) gesture.moved = true;
    if (!gesture.moved) return;
    updatePosition();
    if (!raf) raf = requestAnimationFrame(autoScroll);
  }

  function cancel() {
    gesture = null;
    cancelAnimationFrame(raf);
    raf = 0;
    message = "";
  }

  function commitTask(task: Task, sourceIndex: number, dayIndex: number, time: string, durationMinutes: number) {
    const source = doc.current.days[sourceIndex];
    const target = doc.current.days[dayIndex];
    if (!source || !target) return null;
    if (source.$id === target.$id) {
      doc.change((tx) => {
        const handle = tx.at(task);
        handle.time.set(time);
        handle.durationMinutes.set(durationMinutes);
      });
      return task.$id;
    }
    const inserted = doc.change((tx) => {
      tx.at(source).tasks.remove(task.$id);
      return tx.at(target).tasks.insert({
        time,
        title: task.title,
        done: task.done,
        color: task.color,
        durationMinutes,
      });
    });
    return inserted.id;
  }

  function finish() {
    if (!gesture) return;
    const g = { ...gesture };
    cancel();
    if (!g.moved) {
      if (g.id) {
        const task = doc.current.days[g.source]?.tasks.find((item) => item.$id === g.id);
        if (task) edit(g.source, task);
      } else edit(g.source, undefined, Math.min(g.start, 1380));
      return;
    }
    if (g.mode === "draw") {
      edit(g.day, undefined, g.nextStart ?? 0, g.nextEnd - (g.nextStart ?? 0));
      return;
    }
    const source = doc.current.days[g.source];
    const task = source?.tasks.find((item) => item.$id === g.id);
    if (!task) return;
    const time = g.nextStart === null ? "" : clock(g.nextStart);
    const durationMinutes = g.nextStart === null ? g.end - g.start : g.nextEnd - g.nextStart;
    commitTask(task, g.source, g.day, time, durationMinutes);
    message = `Moved ${task.title} to ${doc.current.days[g.day]?.name}, ${time || "Anytime"}`;
  }

  function keyboard(event: KeyboardEvent, day: number, task: Task, edge?: "start" | "end") {
    if (!event.key.startsWith("Arrow") || !board) return;
    event.preventDefault();
    if (!edge && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
      const next = day + (event.key === "ArrowLeft" ? -1 : 1);
      if (next < 0 || next >= doc.current.days.length) return;
      const source = doc.current.days[day];
      const target = doc.current.days[next];
      if (!source || !target) return;
      const inserted = doc.change((tx) => {
        tx.at(source).tasks.remove(task.$id);
        return tx.at(target).tasks.insert({
          time: task.time,
          title: task.title,
          done: task.done,
          color: task.color,
          ...(task.durationMinutes === undefined ? {} : { durationMinutes: task.durationMinutes }),
        });
      });
      requestAnimationFrame(() =>
        board?.querySelector<HTMLButtonElement>(`[data-task-id="${CSS.escape(inserted.id)}"]`)?.focus(),
      );
      message = `${task.title}, ${target.name}`;
    } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      const start = minutes(task.time);
      if (start === null) return;
      const span = Math.min(duration(task), 1440 - start);
      const step = event.key === "ArrowUp" ? -15 : 15;
      doc.change((tx) => {
        const handle = tx.at(task);
        if (edge) {
          const next = resize(start, start + span, (edge === "start" ? start : start + span) + step, edge);
          handle.time.set(clock(next.start));
          handle.durationMinutes.set(next.end - next.start);
          message = `${task.title}, ${clock(next.start)}`;
        } else {
          const time = clock(move(start + step, span));
          handle.time.set(time);
          message = `${task.title}, ${time}`;
        }
      });
    }
  }

  onDestroy(() => cancelAnimationFrame(raf));
</script>

<svelte:window
  onpointermove={pointerMove}
  onpointerup={finish}
  onpointercancel={cancel}
  onkeydown={(event) => {
    if (event.key === "Escape") cancel();
  }}
/>
<Slop>
  <main class="canvas" aria-label="Weekly planner">
    <header class="header">
      <div class="identity">
        <span class="eyebrow">Weekly planner</span>
        <input class="weekTitle" aria-label="Week title" use:bindText={doc.fields.week} />
      </div>
      <label class="focus">
        THIS WEEK’S FOCUS
        <input aria-label="Weekly focus" placeholder="One thing that matters" use:bindText={doc.fields.focus} />
      </label>
    </header>
    <div class="toolbar">
      <button class="button" onclick={() => { if (scroller) scroller.scrollTop = 0; }}>Anytime tasks</button>
      <button class="primary" onclick={() => edit(0)}>+ Add block</button>
    </div>
    <div class="scroll" bind:this={scroller}>
      <div class="board" bind:this={board}>
        <div class="corner">WEEK</div>
        {#each doc.current.days as day, index (day.$id)}
          <div class="dayHead">
            <div>
              <span class="dayName">{day.name}</span>
              <div class="date">{day.date}</div>
            </div>
            <button class="add" aria-label="Add block on {day.name}" onclick={() => edit(index)}>+</button>
          </div>
        {/each}
        <div class="anytimeLabel">ANYTIME</div>
        {#each doc.current.days as day, index (day.$id)}
          <div class="anytime" data-anytime={index}>
            {#each day.tasks.filter((task) => minutes(task.time) === null) as task (task.$id)}
              <div class="untimed" data-color={task.color} data-done={task.done}>
                <button
                  class="blockBody"
                  data-task-id={task.$id}
                  aria-label="Edit or move {task.title || 'block'}, {day.name}, Anytime"
                  onpointerdown={(event) => begin(event, index, task, "move")}
                  onclick={(event) => { if (event.detail === 0) edit(index, task); }}
                  onkeydown={(event) => keyboard(event, index, task)}
                >{task.done ? "✓ " : ""}{task.title || "Untitled"}</button>
              </div>
            {/each}
            {#if gesture?.moved && gesture.day === index && gesture.nextStart === null}
              <span class="eyebrow">Drop here</span>
            {/if}
          </div>
        {/each}
        <div class="gutter">
          {#each Array.from({ length: 24 }, (_, i) => i) as hour}
            <div class="hour">{label(hour * 60)}</div>
          {/each}
        </div>
        {#each doc.current.days as day, index (day.$id)}
          <div class="lane" data-lane={index}>
            <button
              class="sheet"
              aria-label="Draw a block on {day.name}"
              onpointerdown={(event) => begin(event, index, null, "draw")}
              onclick={(event) => { if (event.detail === 0) edit(index, undefined, 540); }}
            ></button>
            {#each layout(day.tasks) as item (item.task.$id)}
              <article
                class="block"
                data-color={item.task.color}
                data-done={item.task.done}
                data-active={gesture?.id === item.task.$id && gesture.moved}
                style:top={`${(item.start / 60) * HOUR}px`}
                style:height={`${((item.end - item.start) / 60) * HOUR}px`}
                style:left={`calc(${(item.column / item.columns) * 100}% + 2px)`}
                style:width={`calc(${100 / item.columns}% - 4px)`}
              >
                <button
                  class="blockBody"
                  data-short={item.end - item.start <= 30}
                  data-task-id={item.task.$id}
                  aria-label="Edit or move {item.task.title || 'block'}, {day.name}, {label(item.start)} to {label(item.end)}"
                  onpointerdown={(event) => begin(event, index, item.task, "move")}
                  onclick={(event) => { if (event.detail === 0) edit(index, item.task); }}
                  onkeydown={(event) => keyboard(event, index, item.task)}
                >
                  <span class="blockTitle">{item.task.done ? "✓ " : ""}{item.task.title || "Untitled"}</span>
                  {#if item.end - item.start >= 45}
                    <span class="blockTime">{label(item.start)} – {label(item.end)}</span>
                  {/if}
                </button>
                <button
                  class="handle"
                  style:top="0"
                  aria-label="Change start of {item.task.title}"
                  onpointerdown={(event) => begin(event, index, item.task, "start")}
                  onkeydown={(event) => keyboard(event, index, item.task, "start")}
                ></button>
                <button
                  class="handle"
                  style:bottom="0"
                  aria-label="Change end of {item.task.title}"
                  onpointerdown={(event) => begin(event, index, item.task, "end")}
                  onkeydown={(event) => keyboard(event, index, item.task, "end")}
                ></button>
              </article>
            {/each}
            {#if gesture?.moved && gesture.day === index && gesture.nextStart !== null}
              <div
                class="ghost"
                style:top={`${(gesture.nextStart / 60) * HOUR}px`}
                style:height={`${((gesture.nextEnd - gesture.nextStart) / 60) * HOUR}px`}
              >
                {label(gesture.nextStart)}–{label(gesture.nextEnd)}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
    <footer class="footer">
      <span>{doneCount} / {allTasks.length} done</span>
      <span role="status">{message || "Drag to move · Pull edges to resize"}</span>
    </footer>
  </main>
  <Dialog.Root bind:open={dialogOpen}>
    <Dialog.Portal>
      <Dialog.Overlay class="overlay" />
      <Dialog.Content class="dialog">
        <Dialog.Title class="dialogTitle">{editor?.taskId === null ? "Make some room" : "Edit block"}</Dialog.Title>
        <Dialog.Description class="description">A little space for what matters.</Dialog.Description>
        {#if editor}
          <form onsubmit={(event) => { event.preventDefault(); save(); }}>
            <div class="fields">
              <label class="field">
                Title
                <input class="input" aria-label="Block title" bind:value={editor.task.title} placeholder="What’s happening?" required />
              </label>
              <div class="pair">
                <div class="field">
                  <span>Day</span>
                  <Select.Root type="single" bind:value={editor.day} items={dayItems}>
                    <Select.Trigger type="button" class="input" aria-label="Block day">
                      {dayItems.find((item) => item.value === editor?.day)?.label}
                    </Select.Trigger>
                    <Select.Portal>
                      <Select.Content class="selectContent">
                        <Select.Viewport>
                          {#each dayItems as item}
                            <Select.Item class="option" value={item.value} label={item.label}>{item.label}</Select.Item>
                          {/each}
                        </Select.Viewport>
                      </Select.Content>
                    </Select.Portal>
                  </Select.Root>
                </div>
                <div class="field">
                  <span>Color</span>
                  <Select.Root
                    type="single"
                    value={editor.task.color}
                    items={colorItems}
                    onValueChange={(value) => { if (editor && isColor(value)) editor.task.color = value; }}
                  >
                    <Select.Trigger type="button" class="input" aria-label="Block color">{editor.task.color}</Select.Trigger>
                    <Select.Portal>
                      <Select.Content class="selectContent">
                        <Select.Viewport>
                          {#each colorItems as item}
                            <Select.Item class="option" value={item.value} label={item.label}>{item.label}</Select.Item>
                          {/each}
                        </Select.Viewport>
                      </Select.Content>
                    </Select.Portal>
                  </Select.Root>
                </div>
              </div>
              <label class="checkLabel">
                <Checkbox.Root
                  class="check"
                  checked={editor.scheduled}
                  onCheckedChange={(value) => {
                    if (!editor) return;
                    editor.scheduled = value === true;
                    if (editor.scheduled && minutes(editor.task.time) === null) editor.task.time = "09:00";
                  }}
                  aria-label="Schedule a time"
                >{editor.scheduled ? "✓" : ""}</Checkbox.Root>
                Schedule a time
              </label>
              {#if editor.scheduled}
                <div class="pair">
                  <label class="field">
                    Starts
                    <input class="input" type="time" step="900" aria-label="Start time" bind:value={editor.task.time} required />
                  </label>
                  <label class="field">
                    Minutes
                    <input
                      class="input"
                      type="number"
                      min="15"
                      max="1440"
                      step="15"
                      aria-label="Duration in minutes"
                      value={duration(editor.task)}
                      oninput={(event) => { if (editor) editor.task.durationMinutes = event.currentTarget.valueAsNumber; }}
                      required
                    />
                  </label>
                </div>
              {/if}
              <label class="checkLabel">
                <Checkbox.Root class="check" bind:checked={editor.task.done} aria-label="Block completed">
                  {editor.task.done ? "✓" : ""}
                </Checkbox.Root>
                Completed
              </label>
            </div>
            {#if formError}<p class="error" role="alert">{formError}</p>{/if}
            <div class="actions">
              {#if editor.taskId}<button type="button" class="danger" onclick={remove}>Delete</button>{/if}
              <Dialog.Close type="button" class="button">Cancel</Dialog.Close>
              <button type="submit" class="primary">Save block</button>
            </div>
          </form>
        {/if}
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  {#snippet exportView()}
    <article class="exportPage">
      <header class="header">
        <div class="identity">
          <span class="eyebrow">Weekly planner</span>
          <h1 class="weekTitle">{doc.current.week || "This week"}</h1>
        </div>
        <p>{doc.current.focus}</p>
      </header>
      <div class="exportDays">
        {#each doc.current.days as day (day.$id)}
          <section class="exportDay">
            <h2>{day.name} <span class="date">{day.date}</span></h2>
            {#each tasksByTime(day.tasks) as task (task.$id)}
              {@const start = minutes(task.time)}
              <div class="exportBlock" data-color={task.color} data-done={task.done}>
                <strong>{task.done ? "✓ " : ""}{task.title || "Untitled"}</strong><span class="blockTime">{start === null ? "Anytime" : `${label(start)} – ${label(Math.min(1440, start + duration(task)))}`}</span>
              </div>
            {:else}
              <p class="description">Open day</p>
            {/each}
          </section>
        {/each}
      </div>
    </article>
  {/snippet}

  {#snippet icon()}
    <svg class="icon" viewBox="0 0 512 512" width="100%" height="100%" fill="none" aria-hidden="true">
      <rect x="26" y="32" width="460" height="456" rx="48" fill="#172e5530" />
      <rect x="26" y="24" width="460" height="456" rx="48" fill="#edf1f8" />
      <path d="M74 24h364a48 48 0 0 1 48 48v54H26V72a48 48 0 0 1 48-48" fill="#172e55" />
      {#each [96, 176, 256, 336, 416] as x}
        <rect x={x - 15} y="68" width="30" height="10" rx="5" fill="#ffffffa0" />
        <path d={`M${x + 40} 126v322`} stroke="#ccd7e7" />
      {/each}
      {#each [210, 294, 378] as y}
        <path d={`M48 ${y}h416`} stroke="#ccd7e7" />
      {/each}
      {#each [{ x: 58, y: 152, w: 64, h: 112, c: "#c8e7ff" }, { x: 138, y: 236, w: 64, h: 126, c: "#ffd2c8" }, { x: 218, y: 152, w: 64, h: 70, c: "#e0d7fc" }, { x: 298, y: 278, w: 64, h: 144, c: "#c6eedb" }, { x: 378, y: 194, w: 64, h: 84, c: "#c8e7ff" }] as block}
        <rect x={block.x} y={block.y + 5} width={block.w} height={block.h} rx="9" fill="#172e5526" />
        <rect x={block.x} y={block.y} width={block.w} height={block.h} rx="9" fill={block.c} />
        <path d={`M${block.x + 12} ${block.y + 18}h${block.w - 24}`} stroke="#172e5550" stroke-width="5" stroke-linecap="round" />
      {/each}
      {#if allTasks.length > 0 && doneCount === allTasks.length}
        <circle cx="414" cy="408" r="42" fill="#2449a5" />
        <path d="m394 409 14 14 27-30" stroke="white" stroke-width="9" stroke-linecap="round" stroke-linejoin="round" />
      {/if}
    </svg>
  {/snippet}
</Slop>
