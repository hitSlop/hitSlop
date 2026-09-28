<script lang="ts">
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { Dialog, RadioGroup } from "bits-ui";
  import { onDestroy } from "svelte";
  import Plus from "@lucide/svelte/icons/plus";
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import Bell from "@lucide/svelte/icons/bell";
  import Settings2 from "@lucide/svelte/icons/settings-2";
  import Copy from "@lucide/svelte/icons/copy";
  import Star from "@lucide/svelte/icons/star";
  import Check from "@lucide/svelte/icons/check";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import schema, {
    DAYS,
    COLORS,
    isDay,
    isColor,
    isBreakName,
    type ActivityEntry,
    type ClassEntry,
    type DayKey,
    type PeriodSlot,
  } from "./schema";
  import {
    DAY_NAMES,
    COLOR_NAMES,
    clockLabel,
    sortedPeriods,
    periodError,
    validTimes,
    liveStatus,
  } from "./schedule";
  import Choice from "./Choice.svelte";
  import Export from "./Export.svelte";
  import Icon from "./Icon.svelte";

  const doc = useDocument(schema);
  let now = $state(new Date());
  $effect(() => {
    const update = () => {
      now = new Date();
    };
    const interval = setInterval(update, 30000);
    const visible = () => {
      if (document.visibilityState === "visible") update();
    };
    document.addEventListener("visibilitychange", visible);
    return () => {
      clearInterval(interval);
      document.removeEventListener("visibilitychange", visible);
    };
  });
  const status = $derived(liveStatus(doc.current, now));
  const periods = $derived(sortedPeriods(doc.current.periods));
  const days = DAYS.map((value) => ({ value, label: DAY_NAMES[value] }));
  const periodItems = $derived(periods.map((p) => ({ value: p.periodKey, label: `${p.name} · ${clockLabel(p.start)}` })));
  let modal = $state<"class" | "period" | "activity" | "details" | "copy">("class");
  let open = $state(false);
  let fresh = $state(false);
  let error = $state("");
  let confirmDelete = $state(false);
  let confirmCopy = $state(false);
  let showLocker = $state(false);
  let entry = $state({
    $id: "",
    day: "MON",
    periodKey: "",
    subject: "",
    room: "",
    teacher: "",
    color: "slate",
  });
  let period = $state({ $id: "", periodKey: "", name: "", start: "08:00", end: "08:50" });
  let activity = $state({ $id: "", day: "MON", start: "15:30", end: "17:00", title: "", location: "" });
  let details = $state({ studentName: "", term: "", homeroom: "", locker: "" });
  let copyFrom = $state("MON");
  let origin: HTMLElement | null = null;
  const title = $derived(
    modal === "class"
      ? fresh ? "Add a class" : "Your class"
      : modal === "period"
        ? fresh ? "Add a period" : "Bell times"
        : modal === "activity"
          ? fresh ? "After-class plans" : "Edit your plans"
          : modal === "copy"
            ? "Build out your week"
            : "Make it yours",
  );
  function begin(kind: typeof modal, event?: Event) {
    origin = (event?.currentTarget as HTMLElement) || (document.activeElement as HTMLElement);
    modal = kind;
    error = "";
    confirmDelete = false;
    confirmCopy = false;
    open = true;
  }
  function getClass(day: DayKey, periodKey: string) {
    return doc.current.classes.find((c) => c.day === day && c.periodKey === periodKey);
  }
  function editClass(day: DayKey, periodKey: string, event?: Event) {
    const c = getClass(day, periodKey);
    fresh = !c;
    entry = c
      ? { $id: c.$id, day: c.day, periodKey: c.periodKey, subject: c.subject, room: c.room, teacher: c.teacher, color: c.color }
      : { $id: "", day, periodKey, subject: "", room: "", teacher: "", color: "slate" };
    begin("class", event);
  }
  function editPeriod(p?: PeriodSlot, event?: Event) {
    fresh = !p;
    const last = periods.at(-1);
    const start = last?.end || "08:00";
    const [h, m] = start.split(":").map(Number);
    const end = Math.min(h * 60 + m + 50, 1439);
    period = p
      ? { $id: p.$id, periodKey: p.periodKey, name: p.name, start: p.start, end: p.end }
      : {
          $id: "",
          periodKey: crypto.randomUUID(),
          name: `Period ${periods.length + 1}`,
          start,
          end: `${String(Math.floor(end / 60)).padStart(2, "0")}:${String(end % 60).padStart(2, "0")}`,
        };
    begin("period", event);
  }
  function editActivity(day: DayKey, a?: ActivityEntry, event?: Event) {
    fresh = !a;
    activity = a
      ? { $id: a.$id, day: a.day, start: a.start, end: a.end, title: a.title, location: a.location }
      : { $id: "", day, start: "15:30", end: "17:00", title: "", location: "" };
    begin("activity", event);
  }
  function editDetails(event: Event) {
    details = {
      studentName: doc.current.studentName,
      term: doc.current.term,
      homeroom: doc.current.homeroom,
      locker: doc.current.locker,
    };
    showLocker = false;
    begin("details", event);
  }
  const destination = $derived(getClass(entry.day as DayKey, entry.periodKey));
  const willSwap = $derived(destination && destination.$id !== entry.$id);
  function saveClass() {
    if (!entry.subject.trim()) {
      error = "Give this class a name.";
      return false;
    }
    if (!isDay(entry.day) || !isColor(entry.color) || !doc.current.periods.some((p) => p.periodKey === entry.periodKey)) {
      error = "Choose a day and period.";
      return false;
    }
    const day = entry.day;
    const color = entry.color;
    if (fresh) {
      if (destination) {
        error = "That slot is occupied. Choose an empty slot.";
        return false;
      }
      doc.fields.classes.insert({
        periodKey: entry.periodKey,
        day,
        subject: entry.subject.trim(),
        room: entry.room,
        teacher: entry.teacher,
        color,
      });
      return true;
    }
    const row = doc.current.classes.find((c) => c.$id === entry.$id);
    if (!row) return false;
    doc.change((tx) => {
      const other = doc.current.classes.find((c) => c.$id !== row.$id && c.day === day && c.periodKey === entry.periodKey);
      if (other) {
        tx.at(other).day.set(row.day);
        tx.at(other).periodKey.set(row.periodKey);
      }
      const handle = tx.at(row);
      handle.day.set(day);
      handle.periodKey.set(entry.periodKey);
      handle.subject.replace(entry.subject.trim());
      handle.room.replace(entry.room);
      handle.teacher.replace(entry.teacher);
      handle.color.set(color);
    });
    return true;
  }
  function save(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    if (modal === "class") {
      if (!saveClass()) return;
    } else if (modal === "period") {
      error = periodError(period, doc.current.periods);
      if (error) return;
      if (fresh) doc.fields.periods.insert({ periodKey: period.periodKey, name: period.name, start: period.start, end: period.end });
      else {
        const row = doc.current.periods.find((p) => p.$id === period.$id);
        if (row) doc.change((tx) => {
          tx.at(row).name.replace(period.name);
          tx.at(row).start.set(period.start);
          tx.at(row).end.set(period.end);
        });
      }
    } else if (modal === "activity") {
      if (!activity.title.trim()) {
        error = "Give your plan a title.";
        return;
      }
      if (!isDay(activity.day) || !validTimes(activity.start, activity.end)) {
        error = "Choose a day and an end time after the start time.";
        return;
      }
      const day = activity.day;
      if (fresh) {
        doc.fields.activities.insert({ day, start: activity.start, end: activity.end, title: activity.title, location: activity.location });
      } else {
        const row = doc.current.activities.find((a) => a.$id === activity.$id);
        if (row) doc.change((tx) => {
          const handle = tx.at(row);
          handle.day.set(day);
          handle.start.set(activity.start);
          handle.end.set(activity.end);
          handle.title.replace(activity.title);
          handle.location.replace(activity.location);
        });
      }
    } else if (modal === "details") {
      doc.change((tx) => {
        tx.fields.studentName.replace(details.studentName);
        tx.fields.term.replace(details.term);
        tx.fields.homeroom.replace(details.homeroom);
        tx.fields.locker.replace(details.locker);
      });
    } else if (modal === "copy") {
      if (!isDay(copyFrom)) return;
      const day = copyFrom;
      if (!confirmCopy && doc.current.classes.some((c) => c.day !== day)) {
        confirmCopy = true;
        return;
      }
      const source = doc.current.classes.filter((c) => c.day === day);
      doc.change((tx) => {
        for (const dest of DAYS) {
          if (dest === day) continue;
          const matched = new Set<string>();
          for (const row of doc.current.classes.filter((c) => c.day === dest)) {
            const from = source.find((c) => c.periodKey === row.periodKey);
            if (!from) {
              tx.fields.classes.remove(row.$id);
              continue;
            }
            matched.add(from.$id);
            const handle = tx.at(row);
            handle.subject.replace(from.subject);
            handle.room.replace(from.room);
            handle.teacher.replace(from.teacher);
            handle.color.set(from.color);
          }
          for (const from of source) {
            if (matched.has(from.$id)) continue;
            tx.fields.classes.insert({
              periodKey: from.periodKey,
              day: dest,
              subject: from.subject,
              room: from.room,
              teacher: from.teacher,
              color: from.color,
            });
          }
        }
      });
    }
    open = false;
  }
  function remove() {
    if (modal === "class") doc.fields.classes.remove(entry.$id);
    else if (modal === "activity") doc.fields.activities.remove(activity.$id);
    else if (modal === "period") {
      if (!confirmDelete && doc.current.classes.some((c) => c.periodKey === period.periodKey)) {
        confirmDelete = true;
        return;
      }
      const doomed = doc.current.classes.filter((c) => c.periodKey === period.periodKey);
      doc.change((tx) => {
        tx.fields.periods.remove(period.$id);
        for (const row of doomed) tx.fields.classes.remove(row.$id);
      });
    } else return;
    open = false;
  }
  function restoreFocus(event: Event) {
    event.preventDefault();
    (origin?.isConnected ? origin : document.querySelector<HTMLElement>("[data-details]"))?.focus({ preventScroll: true });
  }
  let scroller: HTMLDivElement;
  let drag = $state<{ id: string; day: DayKey; periodKey: string; x: number; y: number; moved: boolean } | null>(null);
  let target = $state<{ day: DayKey; periodKey: string } | null>(null);
  let pointerX = 0;
  let pointerY = 0;
  let raf = 0;
  const dragLabel = $derived(
    target && drag
      ? getClass(target.day, target.periodKey)?.$id === drag.id
        ? "Keep here"
        : getClass(target.day, target.periodKey)
          ? `Swap with ${getClass(target.day, target.periodKey)?.subject}`
          : `Move to ${DAY_NAMES[target.day]}`
      : "",
  );
  function startDrag(event: PointerEvent, c: ClassEntry) {
    if (event.button !== 0) return;
    drag = { id: c.$id, day: c.day, periodKey: c.periodKey, x: event.clientX, y: event.clientY, moved: false };
    pointerX = event.clientX;
    pointerY = event.clientY;
    target = null;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    event.preventDefault();
    raf = requestAnimationFrame(autoScroll);
  }
  function updateTarget() {
    const cell = document.elementFromPoint(pointerX, pointerY)?.closest<HTMLElement>("[data-slot-day]");
    target = cell && isDay(cell.dataset.slotDay || "")
      ? { day: cell.dataset.slotDay as DayKey, periodKey: cell.dataset.slotPeriod! }
      : null;
  }
  function moveDrag(event: PointerEvent) {
    if (!drag) return;
    pointerX = event.clientX;
    pointerY = event.clientY;
    if (Math.hypot(pointerX - drag.x, pointerY - drag.y) > 5) drag.moved = true;
    if (drag.moved) updateTarget();
  }
  function autoScroll() {
    if (!drag) return;
    if (drag.moved && scroller) {
      const r = scroller.getBoundingClientRect();
      scroller.scrollBy(pointerX < r.left + 35 ? -6 : pointerX > r.right - 35 ? 6 : 0, pointerY < r.top + 45 ? -6 : pointerY > r.bottom - 35 ? 6 : 0);
      updateTarget();
    }
    raf = requestAnimationFrame(autoScroll);
  }
  function cancelDrag() {
    cancelAnimationFrame(raf);
    drag = null;
    target = null;
  }
  function endDrag(event: PointerEvent) {
    if (!drag) return;
    const d = drag;
    const t = target;
    cancelDrag();
    if (!d.moved) {
      editClass(d.day, d.periodKey, event);
      return;
    }
    if (!t) return;
    const source = doc.current.classes.find((c) => c.$id === d.id);
    if (!source) return;
    const other = doc.current.classes.find((c) => c.$id !== source.$id && c.day === t.day && c.periodKey === t.periodKey);
    doc.change((tx) => {
      if (other) {
        tx.at(other).day.set(source.day);
        tx.at(other).periodKey.set(source.periodKey);
      }
      tx.at(source).day.set(t.day);
      tx.at(source).periodKey.set(t.periodKey);
    });
  }
  onDestroy(() => cancelAnimationFrame(raf));
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") cancelDrag();
  }}
/>
<Slop>
<main
  class={"sch-canvas"}
  aria-label="School schedule"
  data-slop-selection="none"
>
  <div class={"sch-app"}>
    <header class={"sch-header"}>
      <div class={"sch-identity"}>
        <span class={"sch-eyebrow"}>SCHOOL / SCHEDULE</span><button
          class={"sch-nameButton"}
          data-details
          onclick={editDetails}
          aria-label="Edit student details"
          ><h1 class={"sch-name"}>{doc.current.studentName || "Your week"}</h1>
          <ArrowUpRight size={20} /></button
        >
        <p class={"sch-term"}>{doc.current.term || "Make this term yours."}</p>
      </div>
      <div class={"sch-live"} aria-label="Current schedule status">
        <span class={"sch-statusLabel"}><Bell size={13} />{status.label}</span
        ><strong class={"sch-liveTitle"}>{status.title}</strong><span
          class={"sch-liveDetail"}>{status.detail}</span
        >{#if status.next}<span class={"sch-upNext"}>{status.next}</span>{/if}
      </div>
    </header>
    <div class={"sch-toolbar"}>
      <div class={"sch-boardHeading"}>
        <span class={"sch-starMark"} aria-hidden="true"><Star size={17} /></span>
        <h2 class={"sch-heading"}>A week of possibilities.</h2>
      </div>
      <div class={"sch-tools"}>
        <button
          class={"sch-tool"}
          onclick={(e) => {
            copyFrom = status.day || "MON";
            begin("copy", e);
          }}><Copy size={13} /> Copy day</button
        ><button class={"sch-tool"} onclick={(e) => editPeriod(undefined, e)}
          ><Plus size={14} /> Period</button
        >
      </div>
    </div>
    <div class={"sch-scroller"} bind:this={scroller}>
      <div class={"sch-board"} role="region" aria-label="Weekly timetable">
        <div class={"sch-dayRow"}>
          <div class={"sch-corner"}>BELL TIMES</div>
          {#each DAYS as day}<div
              class={"sch-dayHead"}
              data-today={status.day === day}
            >
              {DAY_NAMES[day]}{#if status.day === day}<span class={"sch-todayBadge"}
                  >TODAY</span
                >{/if}
            </div>{/each}
        </div>
        {#each periods as p (p.$id)}<div class={"sch-periodRow"}>
            <button
              class={"sch-periodCell"}
              onclick={(e) => editPeriod(p, e)}
              aria-label="Edit {p.name} bell times"
              ><strong>{p.name}</strong><span
                >{clockLabel(p.start)}<br />{clockLabel(p.end)}</span
              ></button
            >
            {#each DAYS as day}{@const c = getClass(day, p.periodKey)}{@const nowCell =
                status.day === day && status.periodKey === p.periodKey}
              <div
                class={"sch-slot"}
                data-slot-day={day}
                data-slot-period={p.periodKey}
                data-target={drag?.moved &&
                  target?.day === day &&
                  target?.periodKey === p.periodKey}
                data-today={status.day === day}
              >
                {#if c}<button
                    class={"sch-classTile"}
                    data-color={c.color}
                    data-now={nowCell}
                    data-dragging={drag?.id === c.$id && drag.moved}
                    onpointerdown={(e) => startDrag(e, c)}
                    onpointermove={moveDrag}
                    onpointerup={endDrag}
                    onpointercancel={cancelDrag}
                    onclick={(e) => {
                      if (e.detail === 0) editClass(day, p.periodKey, e);
                    }}
                    aria-label="Edit {c.subject || 'class'}, {DAY_NAMES[
                      day
                    ]}, {p.name}"
                    title="{c.subject} · {c.room} · {c.teacher}"
                    ><strong class={"sch-subject"}
                      >{c.subject || "Untitled class"}</strong
                    ><span class={"sch-room"}>{c.room || "Room to be added"}</span
                    >{#if nowCell}<span class={"sch-nowBadge"}>NOW</span
                      >{/if}</button
                  >
                {:else}<button
                    class={"sch-emptySlot"}
                    onclick={(e) => editClass(day, p.periodKey, e)}
                    aria-label="Add class on {DAY_NAMES[day]}, {p.name}"
                    ><Plus size={13} /><span
                      >{isBreakName(p.name) ? p.name : "Free"}</span
                    ></button
                  >{/if}
                {#if drag?.moved && target?.day === day && target?.periodKey === p.periodKey}<span
                    class={"sch-dropLabel"}>{dragLabel}</span
                  >{/if}
              </div>{/each}
          </div>
        {:else}<div class={"sch-emptyBoard"}>
            <Star size={28} />
            <h2>Your next chapter starts here.</h2>
            <p>Add your bell times, then give each class a place.</p>
            <button class={"sch-primary"} onclick={(e) => editPeriod(undefined, e)}
              >Add first period</button
            >
          </div>{/each}
        <div class={"sch-afterHeading"}>
          <h2 class={"sch-heading"}>After class</h2>
          <span>Clubs, practice & your kind of plans.</span>
        </div>
        <div class={"sch-afterGrid"}>
          <div class={"sch-afterCorner"} aria-hidden="true">
            <ArrowUpRight size={24} />
          </div>
          {#each DAYS as day}<section
              class={"sch-activityColumn"}
              aria-label="{DAY_NAMES[day]} after class"
            >
              {#each doc.current.activities
                .filter((a) => a.day === day)
                .sort( (a, b) => a.start.localeCompare(b.start) ) as a (a.$id)}<button
                  class={"sch-activityTile"}
                  onclick={(e) => editActivity(day, a, e)}
                  ><strong>{a.title || "Your plans"}</strong><span
                    >{clockLabel(a.start)} · {a.location ||
                      "Location TBD"}</span
                  ></button
                >{/each}<button
                class={"sch-addActivity"}
                onclick={(e) => editActivity(day, undefined, e)}
                aria-label="Add after-class plan on {DAY_NAMES[day]}"
                ><Plus size={12} /> Add plan</button
              >
            </section>{/each}
        </div>
      </div>
    </div>
    <footer class={"sch-footer"}>
      <span
        >{doc.current.homeroom
          ? `Homeroom · ${doc.current.homeroom}`
          : "Your week. Your rhythm."}</span
      ><button class={"sch-footerButton"} onclick={editDetails}
        ><Settings2 size={12} /> My details</button
      >
    </footer>
  </div>
  <div class={"sch-srOnly"} aria-live="polite">{drag?.moved ? dragLabel : ""}</div>
</main>
<Dialog.Root bind:open
  ><Dialog.Portal
    ><Dialog.Overlay class={"sch-overlay"} /><Dialog.Content
      class={"sch-dialog"}
      onCloseAutoFocus={restoreFocus}
      ><Dialog.Title class={"sch-dialogTitle"}>{title}</Dialog.Title
      ><Dialog.Description class={"sch-description"}
        >{modal === "class"
          ? "The essentials, all in one place."
          : modal === "copy"
            ? "Repeat one day’s classes across the other four weekdays."
            : modal === "period"
              ? "Bell times apply to every weekday."
              : modal === "activity"
                ? "Make room for the good stuff after class."
                : "A schedule that feels like you."}</Dialog.Description
      >
      <form class={"sch-form"} onsubmit={save}>
        {#if modal === "class"}
          <label class={"sch-formLabel"}
            >Subject<input
              class={"sch-input"}
              bind:value={entry.subject}
              placeholder="Biology, studio, something new…"
              required
            /></label
          >
          <div class={"sch-formRow"}>
            <label class={"sch-formLabel"}
              >Room<input
                class={"sch-input"}
                bind:value={entry.room}
                placeholder="Room or building"
              /></label
            ><label class={"sch-formLabel"}
              >Teacher<input
                class={"sch-input"}
                bind:value={entry.teacher}
                placeholder="Name"
              /></label
            >
          </div>
          <div class={"sch-formRow"}>
            <Choice label="Day" bind:value={entry.day} items={days} /><Choice
              label="Period"
              bind:value={entry.periodKey}
              items={periodItems}
            />
          </div>
          {#if willSwap}<p class={"sch-notice"}>
              {fresh
                ? "This slot is occupied."
                : `Saving swaps places with ${destination?.subject || "the class in this slot"}.`}
            </p>{/if}
          <div class={"sch-formLabel"}>
            <span>Subject color</span><RadioGroup.Root
              class={"sch-palette"}
              bind:value={entry.color}
              aria-label="Subject color"
              >{#each COLORS as color}<RadioGroup.Item
                  class={"sch-colorOption"}
                  value={color}
                  data-color={color}
                  aria-label={COLOR_NAMES[color]}
                  title={COLOR_NAMES[color]}
                  >{#if entry.color === color}<Check
                      size={17}
                    />{/if}</RadioGroup.Item
                >{/each}</RadioGroup.Root
            >
          </div>
        {:else if modal === "period"}
          <label class={"sch-formLabel"}
            >Period name<input
              class={"sch-input"}
              bind:value={period.name}
              required
            /></label
          >
          <div class={"sch-formRow"}>
            <label class={"sch-formLabel"}
              >Starts<input
                type="time"
                class={"sch-input"}
                bind:value={period.start}
                required
              /></label
            ><label class={"sch-formLabel"}
              >Ends<input
                type="time"
                class={"sch-input"}
                bind:value={period.end}
                required
              /></label
            >
          </div>
          {#if confirmDelete}<p class={"sch-notice"} role="alert">
              Remove this period and all {doc.current.classes.filter(
                (c) => c.periodKey === period.periodKey,
              ).length} classes in it? This cannot be undone.
            </p>{/if}
        {:else if modal === "activity"}
          <label class={"sch-formLabel"}
            >Plan<input
              class={"sch-input"}
              bind:value={activity.title}
              placeholder="Robotics club, practice, coffee…"
              required
            /></label
          ><Choice label="Day" bind:value={activity.day} items={days} />
          <div class={"sch-formRow"}>
            <label class={"sch-formLabel"}
              >Starts<input
                type="time"
                class={"sch-input"}
                bind:value={activity.start}
                required
              /></label
            ><label class={"sch-formLabel"}
              >Ends<input
                type="time"
                class={"sch-input"}
                bind:value={activity.end}
                required
              /></label
            >
          </div>
          <label class={"sch-formLabel"}
            >Location<input
              class={"sch-input"}
              bind:value={activity.location}
              placeholder="Where to meet"
            /></label
          >
        {:else if modal === "details"}
          <label class={"sch-formLabel"}
            >Student name<input
              class={"sch-input"}
              bind:value={details.studentName}
              placeholder="Your name"
            /></label
          ><label class={"sch-formLabel"}
            >Term<input
              class={"sch-input"}
              bind:value={details.term}
              placeholder="Fall semester"
            /></label
          ><label class={"sch-formLabel"}
            >Homeroom<input
              class={"sch-input"}
              bind:value={details.homeroom}
              placeholder="Room · teacher"
            /></label
          ><label class={"sch-formLabel"}
            >Locker number & combination<input
              class={"sch-input"}
              type={showLocker ? "text" : "password"}
              bind:value={details.locker}
              autocomplete="off"
            /></label
          ><button
            type="button"
            class={"sch-reveal"}
            onclick={() => (showLocker = !showLocker)}
            >{#if showLocker}<EyeOff size={14} /> Hide locker details{:else}<Eye
                size={14}
              /> Show locker details{/if}</button
          >
        {:else}
          <Choice label="Day to copy" bind:value={copyFrom} items={days} />
          <p class={"sch-notice"}>
            {doc.current.classes.filter((c) => c.day === copyFrom).length} classes
            will be repeated. Empty slots will also replace existing classes. After-class
            plans stay as they are.
          </p>
          {#if confirmCopy}<p class={"sch-error"} role="alert">
              Replace the other four days’ classes with this day? This cannot be
              undone.
            </p>{/if}
        {/if}
        {#if error}<p class={"sch-error"} role="alert">{error}</p>{/if}
        <div class={"sch-actions"}>
          {#if !fresh && ["class", "period", "activity"].includes(modal)}<button
              type="button"
              class={"sch-deleteButton sch-secondary sch-primary"}
              onclick={remove}>{confirmDelete ? "Remove all" : "Delete"}</button
            >{/if}<Dialog.Close class={"sch-secondary sch-primary"} type="button"
            >Cancel</Dialog.Close
          ><button class={"sch-primary"} type="submit"
            >{modal === "copy"
              ? confirmCopy
                ? "Replace & copy"
                : "Copy across week"
              : "Save changes"}</button
          >
        </div>
      </form></Dialog.Content
    ></Dialog.Portal
  ></Dialog.Root
>

{#snippet exportView()}
  <Export />
{/snippet}
{#snippet icon()}
  <Icon />
{/snippet}
</Slop>
