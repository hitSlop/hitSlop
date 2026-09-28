<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import Calendar from "@lucide/svelte/icons/calendar";
  import Flag from "@lucide/svelte/icons/flag";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { Calendar as CalendarPrimitive, Popover, Select } from "bits-ui";
  import { CalendarDate, getLocalTimeZone, today as getTodayDate, type DateValue } from "@internationalized/date";
  import schema, { type Milestone, type MilestoneType } from "./schema";

  const MILESTONE_TYPES: { type: MilestoneType; label: string; color: string }[] = [
    { type: "Exam", label: "Exam / Midterm", color: "#dc2626" },
    { type: "Paper", label: "Major Paper", color: "#d97706" },
    { type: "Project", label: "Project / Deliverable", color: "#4f46e5" },
    { type: "Presentation", label: "Presentation", color: "#0d9488" },
    { type: "Break", label: "Break / Recess", color: "#16a34a" },
    { type: "Deadline", label: "Administrative Deadline", color: "#4a5d6e" },
  ];

  const MONTHS = [
    { year: 2026, month: 8, name: "September", days: 30 },
    { year: 2026, month: 9, name: "October", days: 31 },
    { year: 2026, month: 10, name: "November", days: 30 },
    { year: 2026, month: 11, name: "December", days: 31 },
  ];

  const doc = useDocument(schema);

  let selectedDate = $state<string | null>(null);
  let filterCourse = $state("ALL");
  let draftDate = $state("2026-09-15");
  let draftTitle = $state("");
  let draftCourse = $state("BIO");
  let draftType = $state<MilestoneType>("Exam");
  let datePopoverOpen = $state(false);

  const draftCalendarValue = $derived.by(() => {
    try {
      const [year, month, day] = draftDate.split("-").map(Number);
      return new CalendarDate(year ?? 2026, month ?? 9, day ?? 15);
    } catch {
      return getTodayDate(getLocalTimeZone());
    }
  });

  function onDraftDateSelect(date: DateValue | undefined): void {
    if (!date) return;
    draftDate = `${date.year}-${String(date.month).padStart(2, "0")}-${String(date.day).padStart(2, "0")}`;
    datePopoverOpen = false;
  }

  function pad(n: number): string {
    return String(n).padStart(2, "0");
  }

  function formatDateStr(year: number, monthIdx: number, day: number): string {
    return `${year}-${pad(monthIdx + 1)}-${pad(day)}`;
  }

  function getMilestonesForDate(dateStr: string): Milestone[] {
    return doc.current.milestones.filter((milestone) => milestone.date === dateStr);
  }

  const sortedMilestones = $derived(
    [...doc.current.milestones]
      .filter((milestone) => filterCourse === "ALL" || milestone.courseCode === filterCourse || milestone.courseCode === "ALL")
      .sort((a, b) => a.date.localeCompare(b.date)),
  );

  const nextMilestone = $derived.by(() => {
    const today = "2026-09-02";
    return [...doc.current.milestones].filter((milestone) => milestone.date >= today).sort((a, b) => a.date.localeCompare(b.date))[0] ?? null;
  });

  function daysUntil(dateStr: string): number {
    const today = new Date("2026-09-02T00:00:00");
    const target = new Date(dateStr + "T00:00:00");
    return Math.round((target.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
  }

  function getTypeColor(type: MilestoneType): string {
    return MILESTONE_TYPES.find((item) => item.type === type)?.color || "#4a5d6e";
  }

  function getCourseColor(code: string): string {
    return doc.current.courses.find((course) => course.code === code)?.colorHex || "#4a5d6e";
  }

  function addMilestone() {
    if (!draftTitle.trim()) return;
    doc.fields.milestones.insert({
      date: draftDate,
      title: draftTitle.trim(),
      courseCode: draftCourse,
      type: draftType,
      notes: "",
    });
    draftTitle = "";
  }

  function removeMilestone(id: string) {
    doc.fields.milestones.remove(id);
  }
</script>

<Slop>
<main class="semester-app">
  <div class="semester-canvas">
    <!-- Header -->
    <header class="semester-header">
      <div class="title-block">
        <div class="sub-label">TERM SYLLABUS & EXAM ROADMAP</div>
        <div class="main-title-row">
          <input class="term-title-input" use:bindText={doc.fields.termTitle} placeholder="Semester Name" />
          <span class="dot">·</span>
          <input class="year-input" use:bindText={doc.fields.academicYear} placeholder="Academic Year" />
        </div>
      </div>

      <!-- Next Milestone Countdown -->
      {#if nextMilestone}
        {@const diff = daysUntil(nextMilestone.date)}
        <div class="next-banner" data-slop-export="hide">
          <Flag size={13} color={getTypeColor(nextMilestone.type)} />
          <div class="next-text">
            <span class="next-head">Next Up:</span>
            <span class="next-title">{nextMilestone.title}</span>
            <span class="next-countdown">in {diff} days ({nextMilestone.date})</span>
          </div>
        </div>
      {/if}

      <!-- Course Filter -->
      <div class="course-filter-wrap" data-slop-export="hide">
        <Select.Root
          type="single"
          bind:value={filterCourse}
        >
          <Select.Trigger class="filter-select" aria-label="Course filter">
            <span>{filterCourse === "ALL" ? `All Courses (${doc.current.milestones.length})` : filterCourse}</span>
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="planner-select-content" data-slop-export="hide">
              <Select.Viewport>
                <Select.Item value="ALL" label="All Courses" class="planner-select-item">
                  {#snippet children({ selected })}
                    <span>All Courses ({doc.current.milestones.length})</span>
                    {#if selected}<Check size={12} />{/if}
                  {/snippet}
                </Select.Item>
                {#each doc.current.courses as c (c.$id)}
                  {#if c.code !== "ALL"}
                    <Select.Item value={c.code} label="{c.code} · {c.name}" class="planner-select-item">
                      {#snippet children({ selected })}
                        <span>{c.code} · {c.name}</span>
                        {#if selected}<Check size={12} />{/if}
                      {/snippet}
                    </Select.Item>
                  {/if}
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      </div>
    </header>

    <!-- 4-Month Calendar Rail -->
    <section class="months-rail">
      {#each MONTHS as m}
        <div class="month-column">
          <div class="month-header">
            <span class="month-name">{m.name}</span>
            <span class="month-year">{m.year}</span>
          </div>
          <div class="month-days-grid">
            {#each Array.from({ length: m.days }, (_, i) => i + 1) as dayNum}
              {@const dateStr = formatDateStr(m.year, m.month, dayNum)}
              {@const dayMilestones = getMilestonesForDate(dateStr)}
              {@const isSelected = selectedDate === dateStr}
              {@const isToday = dateStr === "2026-09-02"}
              <button
                type="button"
                class="day-box"
                class:has-milestone={dayMilestones.length > 0}
                class:is-today={isToday}
                class:selected={isSelected}
                onclick={() => { selectedDate = isSelected ? null : dateStr; draftDate = dateStr; }}
                title="{dateStr}: {dayMilestones.map(x => x.title).join(', ') || 'No milestones'}"
              >
                <span class="day-num">{dayNum}</span>
                {#if dayMilestones.length > 0}
                  <div class="dots-row">
                    {#each dayMilestones as ms}
                      <span
                        class="milestone-dot"
                        style="background-color: {getTypeColor(ms.type)};"
                      ></span>
                    {/each}
                  </div>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </section>

    <!-- Lower Section: Quick Add Bar & Agenda List -->
    <section class="bottom-section">
      <!-- Quick Add Bar -->
      <form class="quick-add-form" onsubmit={(e) => { e.preventDefault(); addMilestone(); }} data-slop-export="hide">
        <Popover.Root bind:open={datePopoverOpen}>
          <Popover.Trigger class="date-trigger-btn" aria-label="Select date">
            <Calendar size={12} />
            <span>{draftDate || "Select Date"}</span>
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Content class="planner-calendar-popover" side="top" align="start" sideOffset={6} data-slop-export="hide">
              <CalendarPrimitive.Root
                type="single"
                value={draftCalendarValue}
                onValueChange={onDraftDateSelect}
              >
                {#snippet children({ months, weekdays })}
                  <CalendarPrimitive.Header class="slop-cal-header">
                    <CalendarPrimitive.PrevButton class="slop-cal-nav" aria-label="Previous month">
                      <ChevronLeft size={13} />
                    </CalendarPrimitive.PrevButton>
                    <CalendarPrimitive.Heading class="slop-cal-title" />
                    <CalendarPrimitive.NextButton class="slop-cal-nav" aria-label="Next month">
                      <ChevronRight size={13} />
                    </CalendarPrimitive.NextButton>
                  </CalendarPrimitive.Header>
                  {#each months as month}
                    <CalendarPrimitive.Grid class="slop-cal-grid">
                      <CalendarPrimitive.GridHead>
                        <CalendarPrimitive.GridRow class="slop-cal-row">
                          {#each weekdays as day}
                            <CalendarPrimitive.HeadCell class="slop-cal-head-cell">{day.slice(0, 2)}</CalendarPrimitive.HeadCell>
                          {/each}
                        </CalendarPrimitive.GridRow>
                      </CalendarPrimitive.GridHead>
                      <CalendarPrimitive.GridBody>
                        {#each month.weeks as weekDates}
                          <CalendarPrimitive.GridRow class="slop-cal-row">
                            {#each weekDates as date}
                              <CalendarPrimitive.Cell {date} month={month.value} class="slop-cal-cell">
                                <CalendarPrimitive.Day class="slop-cal-day" />
                              </CalendarPrimitive.Cell>
                            {/each}
                          </CalendarPrimitive.GridRow>
                        {/each}
                      </CalendarPrimitive.GridBody>
                    </CalendarPrimitive.Grid>
                  {/each}
                {/snippet}
              </CalendarPrimitive.Root>
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
        <Select.Root
          type="single"
          bind:value={draftType}
        >
          <Select.Trigger class="type-select" aria-label="Milestone type">
            <span>{draftType}</span>
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="planner-select-content" data-slop-export="hide">
              <Select.Viewport>
                {#each MILESTONE_TYPES as mt}
                  <Select.Item value={mt.type} label={mt.label} class="planner-select-item">
                    {#snippet children({ selected })}
                      <span>{mt.label}</span>
                      {#if selected}<Check size={12} />{/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
        <Select.Root
          type="single"
          bind:value={draftCourse}
        >
          <Select.Trigger class="course-select" aria-label="Course">
            <span>{draftCourse}</span>
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="planner-select-content" data-slop-export="hide">
              <Select.Viewport>
                {#each doc.current.courses as c (c.$id)}
                  <Select.Item value={c.code} label={c.code} class="planner-select-item">
                    {#snippet children({ selected })}
                      <span>{c.code}</span>
                      {#if selected}<Check size={12} />{/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
        <input class="title-input" bind:value={draftTitle} placeholder="Milestone / Exam / Project title..." />
        <button type="submit" class="add-btn" title="Add milestone">
          <Plus size={13} />
          <span>Add Date</span>
        </button>
      </form>

      <!-- Chronological Agenda Table -->
      <div class="agenda-container">
        <table class="agenda-table">
          <thead>
            <tr>
              <th style="width: 90px;">Date</th>
              <th style="width: 70px;">Course</th>
              <th style="width: 110px;">Type</th>
              <th>Milestone / Description</th>
              <th style="width: 140px;">Notes / Prep</th>
              <th style="width: 30px;" data-slop-export="hide"></th>
            </tr>
          </thead>
          <tbody>
            {#each sortedMilestones as ms (ms.$id)}
              {@const isSelected = selectedDate === ms.date}
              <tr class="agenda-row" class:selected={isSelected}>
                <td class="date-cell">
                  <input class="cell-input mono" use:bindValue={doc.at(ms).date} />
                </td>
                <td>
                  <span class="course-badge" style="background-color: {getCourseColor(ms.courseCode)};">
                    {ms.courseCode}
                  </span>
                </td>
                <td>
                  <span class="type-badge" style="color: {getTypeColor(ms.type)}; border-color: {getTypeColor(ms.type)};">
                    {ms.type}
                  </span>
                </td>
                <td>
                  <input class="cell-input bold" use:bindText={doc.at(ms).title} />
                </td>
                <td>
                  <input class="cell-input muted" use:bindText={doc.at(ms).notes} placeholder="Add prep notes..." />
                </td>
                <td data-slop-export="hide">
                  <button
                    type="button"
                    class="del-btn"
                    onclick={() => removeMilestone(ms.$id)}
                    title="Remove date"
                  >
                    <Trash2 size={11} />
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>

    <!-- Legend Footer -->
    <footer class="semester-footer">
      <div class="type-legend">
        {#each MILESTONE_TYPES as mt}
          <div class="legend-item">
            <span class="legend-dot" style="background-color: {mt.color};"></span>
            <span class="legend-label">{mt.label}</span>
          </div>
        {/each}
      </div>
      <div class="total-count">
        <span>{doc.current.milestones.length} Term Milestones</span>
      </div>
    </footer>
  </div>
</main>

{#snippet exportView()}
  <main class="semester-app">
    <div class="semester-canvas">
      <header class="semester-header">
        <div class="title-block">
          <div class="sub-label">TERM SYLLABUS & EXAM ROADMAP</div>
          <div class="main-title-row">
            <h1 class="term-title-input">{doc.current.termTitle}</h1>
            <span class="dot">·</span>
            <p class="year-input">{doc.current.academicYear}</p>
          </div>
        </div>
      </header>
      <section class="months-rail">
        {#each MONTHS as month}
          <div class="month-column">
            <div class="month-header"><span class="month-name">{month.name}</span><span class="month-year">{month.year}</span></div>
            <div class="month-days-grid">
              {#each Array.from({ length: month.days }, (_, i) => i + 1) as dayNum}
                {@const dateStr = formatDateStr(month.year, month.month, dayNum)}
                {@const dayMilestones = getMilestonesForDate(dateStr)}
                <div class="day-box" class:has-milestone={dayMilestones.length > 0}>
                  <span class="day-num">{dayNum}</span>
                  {#if dayMilestones.length > 0}
                    <div class="dots-row">
                      {#each dayMilestones as ms (ms.$id)}<span class="milestone-dot" style="background-color: {getTypeColor(ms.type)};"></span>{/each}
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </section>
      <div class="agenda-container">
        <table class="agenda-table">
          <tbody>
            {#each sortedMilestones as ms (ms.$id)}
              <tr class="agenda-row">
                <td class="date-cell">{ms.date}</td>
                <td><span class="course-badge" style="background-color: {getCourseColor(ms.courseCode)};">{ms.courseCode}</span></td>
                <td><span class="type-badge" style="color: {getTypeColor(ms.type)}; border-color: {getTypeColor(ms.type)};">{ms.type}</span></td>
                <td>{ms.title}</td>
                <td>{ms.notes}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  </main>
{/snippet}

{#snippet icon()}
<section class="semester-render semester-icon" data-slop-render="icon" aria-hidden="true">
  <div class="icon-canvas">
    <div class="icon-header">
      <span class="icon-term-bar"></span>
      <span class="icon-badge"></span>
    </div>
    <div class="icon-months-grid">
      <div class="icon-month-card">
        <span class="icon-m-title"></span>
        <div class="icon-mini-grid">
          <span class="dot"></span><span class="dot"></span><span class="dot dot-exam"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot dot-proj"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot dot-exam"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
        </div>
      </div>
      <div class="icon-month-card">
        <span class="icon-m-title"></span>
        <div class="icon-mini-grid">
          <span class="dot"></span><span class="dot dot-paper"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot dot-exam"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot dot-proj"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
        </div>
      </div>
      <div class="icon-month-card">
        <span class="icon-m-title"></span>
        <div class="icon-mini-grid">
          <span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot dot-exam"></span>
          <span class="dot dot-break"></span><span class="dot dot-break"></span><span class="dot dot-break"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot dot-paper"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
        </div>
      </div>
      <div class="icon-month-card">
        <span class="icon-m-title"></span>
        <div class="icon-mini-grid">
          <span class="dot"></span><span class="dot dot-proj"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot dot-exam"></span><span class="dot dot-exam"></span><span class="dot dot-exam"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot dot-break"></span><span class="dot dot-break"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
          <span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span><span class="dot"></span>
        </div>
      </div>
    </div>
    <div class="icon-agenda-bar">
      <span class="icon-agenda-item"></span>
      <span class="icon-agenda-item short"></span>
    </div>
  </div>
</section>
{/snippet}
</Slop>
