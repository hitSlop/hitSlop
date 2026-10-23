<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { Dialog, Select, Checkbox } from "bits-ui";
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText, getDocument, getEnvironment, haptics } from "hitslop/svelte";
  import schema from "./schema";
  import {
    type Task,
    type TaskColor,
    addBlock,
    updateBlock,
    removeBlock,
    moveBlock,
    nudgeBlock,
    shiftDay,
    plannerView,
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

  const doc = getDocument(schema);
  const environment = getEnvironment();
  // A phone shows one day at a time with taps instead of drag gestures. The selected day is
  // local view state and is never written to the document.
  const compact = $derived(environment.current.layout === "compact");
  const hour = $derived(compact ? 80 : HOUR);
  const weekdays = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
  let pickedDay = $state<number | null>(null);
  const activeDay = $derived.by(() => {
    const days = doc.current.days;
    if (pickedDay !== null) return Math.min(pickedDay, days.length - 1);
    return Math.max(0, days.findIndex((day) => day.name === weekdays[new Date().getDay()]));
  });
  function pickDay(index: number) {
    if (index === activeDay) return;
    pickedDay = index;
    haptics.selection();
  }
  let scroller = $state<HTMLDivElement>();
  let board = $state<HTMLDivElement>();

  onMount(() => {
    void tick().then(() =>
      requestAnimationFrame(() => {
        const lane = board?.querySelector<HTMLElement>("[data-lane]");
        if (lane && scroller) scroller.scrollTop = lane.offsetTop + 8 * hour - (compact ? 8 : 57);
      }),
    );
  });

  const { tasks: allTasks, done: doneCount } = $derived(plannerView(doc.current));
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

  async function save() {
    const draft = editor;
    if (!draft) return;
    const input = {
      day: draft.day,
      title: draft.task.title,
      time: draft.scheduled ? draft.task.time : "",
      done: draft.task.done,
      color: draft.task.color,
      durationMinutes: boundedDuration(draft.task.durationMinutes),
    };
    if (!input.title.trim()) {
      formError = "Give this block a title.";
      return;
    }
    const start = minutes(input.time);
    if (draft.scheduled && (start === null || input.durationMinutes === undefined || start + input.durationMinutes > 1440)) {
      formError = "Choose a valid start and a duration that ends by midnight.";
      return;
    }
    const taskId = draft.taskId;
    const wasDone = allTasks.find(task => task.$id === taskId)?.done ?? false;
    await doc.change((tx) =>
      taskId ? updateBlock(tx, doc.current, taskId, input) : addBlock(tx, doc.current, input),
    );
    dialogOpen = false;
    if (input.done && !wasDone) haptics.notification("success");
    else haptics.impact();
  }

  async function remove() {
    const taskId = editor?.taskId;
    if (!taskId) return;
    await doc.change((tx) => removeBlock(tx, doc.current, taskId));
    haptics.impact("medium");
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
      at: Math.max(0, Math.min(1440, snap(((y - first.top) / hour) * 60))),
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

  async function commitTask(task: Task, dayIndex: number, time: string, durationMinutes: number) {
    const target = doc.current.days[dayIndex];
    if (!target) return null;
    return doc.change((tx) => moveBlock(tx, doc.current, task.$id, target.name, time, durationMinutes));
  }

  async function finish() {
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
    if (g.day === g.source && time === task.time && durationMinutes === duration(task)) return;
    await commitTask(task, g.day, time, durationMinutes);
    haptics.impact();
    message = `Moved ${task.title} to ${doc.current.days[g.day]?.name}, ${time || "Anytime"}`;
  }

  function tapLane(event: MouseEvent, day: number) {
    if (event.detail === 0) return edit(day, undefined, 540);
    const lane = event.currentTarget as HTMLElement;
    const at = snap(((event.clientY - lane.getBoundingClientRect().top) / hour) * 60);
    edit(day, undefined, Math.max(0, Math.min(1380, at)));
  }

  async function keyboard(event: KeyboardEvent, day: number, task: Task, edge?: "start" | "end") {
    if (!event.key.startsWith("Arrow") || !board) return;
    event.preventDefault();
    if (!edge && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
      if (compact) return;
      const direction = event.key === "ArrowLeft" ? -1 : 1;
      const target = doc.current.days[day + direction];
      if (!target) return;
      const { id } = await doc.change((tx) => shiftDay(tx, doc.current, task.$id, direction));
      requestAnimationFrame(() =>
        board?.querySelector<HTMLButtonElement>(`[data-task-id="${CSS.escape(id)}"]`)?.focus(),
      );
      message = `${task.title}, ${target.name}`;
      haptics.impact();
    } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      if (minutes(task.time) === null) return;
      const start = await doc.change((tx) => nudgeBlock(tx, task, event.key === "ArrowUp" ? -15 : 15, edge));
      message = `${task.title}, ${clock(start)}`;
      haptics.selection();
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
      <button
        class="primary"
        aria-label={compact ? `Add block on ${doc.current.days[activeDay]?.name}` : undefined}
        onclick={() => edit(compact ? activeDay : 0)}>+ Add block</button>
    </div>
    {#if compact}
      <div class="dayTabs" role="group" aria-label="Day of the week">
        {#each doc.current.days as day, index (day.$id)}
          <button
            class="dayTab"
            aria-pressed={index === activeDay}
            aria-label="{day.name} {day.date}"
            onclick={() => pickDay(index)}
          >
            <span class="dayTabName">{day.name}</span>
            <span class="dayTabDate">{day.date}</span>
          </button>
        {/each}
      </div>
    {/if}
    <div class="scroll" bind:this={scroller}>
      <div class="board" bind:this={board} style:--hour="{hour}px">
        {#if !compact}<div class="corner">WEEK</div>{/if}
        {#each doc.current.days as day, index (day.$id)}
          {#if !compact}<div class="dayHead">
            <div>
              <span class="dayName">{day.name}</span>
              <div class="date">{day.date}</div>
            </div>
            <button class="add" aria-label="Add block on {day.name}" onclick={() => edit(index)}>+</button>
          </div>{/if}
        {/each}
        <div class="anytimeLabel">ANYTIME</div>
        {#each doc.current.days as day, index (day.$id)}
          {#if !compact || index === activeDay}<div class="anytime" data-anytime={index}>
            {#each day.tasks.filter((task) => minutes(task.time) === null) as task (task.$id)}
              <div class="untimed" data-color={task.color} data-done={task.done}>
                <button
                  class="blockBody"
                  data-task-id={task.$id}
                  aria-label="Edit or move {task.title || 'block'}, {day.name}, Anytime"
                  onpointerdown={compact ? undefined : (event) => begin(event, index, task, "move")}
                  onclick={(event) => { if (compact || event.detail === 0) edit(index, task); }}
                  onkeydown={(event) => keyboard(event, index, task)}
                >{task.done ? "✓ " : ""}{task.title || "Untitled"}</button>
              </div>
            {/each}
            {#if gesture?.moved && gesture.day === index && gesture.nextStart === null}
              <span class="eyebrow">Drop here</span>
            {/if}
          </div>{/if}
        {/each}
        <div class="gutter">
          {#each Array.from({ length: 24 }, (_, i) => i) as hour}
            <div class="hour">{label(hour * 60)}</div>
          {/each}
        </div>
        {#each doc.current.days as day, index (day.$id)}
          {#if !compact || index === activeDay}<div class="lane" data-lane={index}>
            <button
              class="sheet"
              aria-label={compact ? `Add a block on ${day.name}` : `Draw a block on ${day.name}`}
              onpointerdown={compact ? undefined : (event) => begin(event, index, null, "draw")}
              onclick={(event) => { if (compact) tapLane(event, index); else if (event.detail === 0) edit(index, undefined, 540); }}
            ></button>
            {#each layout(day.tasks) as item (item.task.$id)}
              <article
                class="block"
                data-color={item.task.color}
                data-done={item.task.done}
                data-active={gesture?.id === item.task.$id && gesture.moved}
                style:top={`${(item.start / 60) * hour}px`}
                style:height={`${((item.end - item.start) / 60) * hour}px`}
                style:left={`calc(${(item.column / item.columns) * 100}% + 2px)`}
                style:width={`calc(${100 / item.columns}% - 4px)`}
              >
                <button
                  class="blockBody"
                  data-short={item.end - item.start <= 30}
                  data-task-id={item.task.$id}
                  aria-label="Edit or move {item.task.title || 'block'}, {day.name}, {label(item.start)} to {label(item.end)}"
                  onpointerdown={compact ? undefined : (event) => begin(event, index, item.task, "move")}
                  onclick={(event) => { if (compact || event.detail === 0) edit(index, item.task); }}
                  onkeydown={(event) => keyboard(event, index, item.task)}
                >
                  <span class="blockTitle">{item.task.done ? "✓ " : ""}{item.task.title || "Untitled"}</span>
                  {#if item.end - item.start >= 45}
                    <span class="blockTime">{label(item.start)} – {label(item.end)}</span>
                  {/if}
                </button>
                {#if !compact}<button
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
                ></button>{/if}
              </article>
            {/each}
            {#if gesture?.moved && gesture.day === index && gesture.nextStart !== null}
              <div
                class="ghost"
                style:top={`${(gesture.nextStart / 60) * hour}px`}
                style:height={`${((gesture.nextEnd - gesture.nextStart) / 60) * hour}px`}
              >
                {label(gesture.nextStart)}–{label(gesture.nextEnd)}
              </div>
            {/if}
          </div>{/if}
        {/each}
      </div>
    </div>
    <footer class="footer">
      <span>{doneCount} / {allTasks.length} done</span>
      <span role="status">{message || (compact ? "Tap a block to edit" : "Drag to move · Pull edges to resize")}</span>
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
