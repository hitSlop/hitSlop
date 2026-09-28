<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { Calendar, Popover } from "bits-ui";
  import { getLocalTimeZone, today, type DateValue } from "@internationalized/date";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import GraduationCap from "@lucide/svelte/icons/graduation-cap";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import CalendarIcon from "@lucide/svelte/icons/calendar";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import schema from "./schema";

  function autosize(node: HTMLTextAreaElement) {
    const resize = () => {
      node.style.height = "auto";
      node.style.height = `${node.scrollHeight}px`;
    };
    resize();
    node.addEventListener("input", resize);
    return {
      update: resize,
      destroy: () => node.removeEventListener("input", resize),
    };
  }

  const doc = useDocument(schema);

  let studyMode = $state(false);
  let revealedItems = $state<Record<string, boolean>>({});
  let calendarOpen = $state(false);
  let calendarValue = $state<DateValue | undefined>(today(getLocalTimeZone()));

  const currentTheme = $derived(doc.current.theme);
  const totalCount = $derived(doc.current.items.length);
  const revealedCount = $derived(doc.current.items.filter((item) => revealedItems[item.$id]).length);

  function toggleTheme() {
    doc.fields.theme.set(currentTheme === "dark" ? "light" : "dark");
  }

  function toggleStudyMode() {
    studyMode = !studyMode;
    if (studyMode) revealedItems = {};
  }

  function toggleReveal(id: string) {
    revealedItems[id] = !revealedItems[id];
  }

  function revealAll() {
    const next: Record<string, boolean> = {};
    for (const item of doc.current.items) next[item.$id] = true;
    revealedItems = next;
  }

  function hideAll() {
    revealedItems = {};
  }

  function addItem() {
    doc.fields.items.insert({
      cue: "New Question or Key Term",
      notes: "- Detailed notes and supporting facts...",
    });
  }

  function removeItem(id: string) {
    doc.fields.items.remove(id);
  }

  function onDateSelect(val: DateValue | undefined) {
    if (!val) return;
    calendarValue = val;
    doc.fields.date.set(val.toDate(getLocalTimeZone()).toISOString().slice(0, 10));
    calendarOpen = false;
  }
</script>

<Slop>
<main class="cornell-page" class:theme-dark={currentTheme === "dark"}>
  <!-- Binder Margin Strip (Tactile notebook edge) -->
  <aside class="binder-margin" aria-hidden="true" data-slop-export="hide">
    <div class="ring-hole"></div>
    <div class="ring-hole"></div>
    <div class="ring-hole"></div>
  </aside>

  <!-- Main Collegiate Notebook Sheet -->
  <div class="notebook-sheet">
    <!-- Header: University / Course Strip & Topic -->
    <header class="sheet-header">
      <div class="header-top-bar">
        <div class="course-meta">
          <div class="course-badge">
            <input
              class="course-input"
              use:bindText={doc.fields.course}
              placeholder="Course & Code"
              aria-label="Course"
            />
          </div>
          <span class="meta-sep" aria-hidden="true">·</span>
          <div class="meta-item">
            <span class="meta-label">INSTRUCTOR</span>
            <input
              class="meta-input"
              use:bindText={doc.fields.lecturer}
              placeholder="Lecturer Name"
              aria-label="Instructor"
            />
          </div>
          <span class="meta-sep" aria-hidden="true">·</span>

          <!-- Bits UI Calendar Popover Picker -->
          <Popover.Root bind:open={calendarOpen}>
            <Popover.Trigger class="date-trigger" aria-label="Select date">
              <CalendarIcon size={12} />
              <span class="meta-label">DATE</span>
              <strong class="date-text">{doc.current.date}</strong>
            </Popover.Trigger>
            <Popover.Content class="calendar-popover" side="bottom" align="start" sideOffset={6}>
              <Calendar.Root
                type="single"
                value={calendarValue}
                onValueChange={onDateSelect}
              >
                {#snippet children({ months, weekdays })}
                  <Calendar.Header class="cal-header">
                    <Calendar.PrevButton class="cal-nav-btn" aria-label="Previous month">
                      <ChevronLeft size={14} />
                    </Calendar.PrevButton>
                    <Calendar.Heading class="cal-heading" />
                    <Calendar.NextButton class="cal-nav-btn" aria-label="Next month">
                      <ChevronRight size={14} />
                    </Calendar.NextButton>
                  </Calendar.Header>
                  {#each months as month}
                    <Calendar.Grid class="cal-grid">
                      <Calendar.GridHead>
                        <Calendar.GridRow class="cal-row">
                          {#each weekdays as day}
                            <Calendar.HeadCell class="cal-head-cell">{day.slice(0, 2)}</Calendar.HeadCell>
                          {/each}
                        </Calendar.GridRow>
                      </Calendar.GridHead>
                      <Calendar.GridBody>
                        {#each month.weeks as weekDates}
                          <Calendar.GridRow class="cal-row">
                            {#each weekDates as date}
                              <Calendar.Cell {date} month={month.value} class="cal-cell">
                                <Calendar.Day class="cal-day" />
                              </Calendar.Cell>
                            {/each}
                          </Calendar.GridRow>
                        {/each}
                      </Calendar.GridBody>
                    </Calendar.Grid>
                  {/each}
                {/snippet}
              </Calendar.Root>
            </Popover.Content>
          </Popover.Root>
        </div>

        <div class="header-actions" data-slop-export="hide">
          <button
            type="button"
            class="study-btn"
            class:is-active={studyMode}
            onclick={toggleStudyMode}
            title="Toggle active-recall study mask"
          >
            <GraduationCap size={13} />
            <span>{studyMode ? "Study Active" : "Study Mode"}</span>
          </button>
          <button
            type="button"
            class="theme-btn"
            onclick={toggleTheme}
            aria-label={`Switch to ${currentTheme === "dark" ? "light" : "dark"} mode`}
            title={`Switch to ${currentTheme === "dark" ? "light" : "dark"} mode`}
          >
            {#if currentTheme === "dark"}
              <Sun size={13} />
            {:else}
              <Moon size={13} />
            {/if}
          </button>
        </div>
      </div>

      <div class="topic-row">
        <textarea
          class="topic-input"
          rows={1}
          use:bindText={doc.fields.topic}
          use:autosize
          placeholder="Lecture Topic / Subject Chapter"
          aria-label="Lecture Topic"
        ></textarea>
      </div>

      <!-- Active Recall Study Toolbar -->
      {#if studyMode}
        <div class="study-bar" data-slop-export="hide">
          <div class="study-bar-info">
            <span class="study-dot" aria-hidden="true"></span>
            <strong>Active Recall:</strong>
            <span>Cover notes and quiz yourself using the cues.</span>
            <span class="study-progress-pill">{revealedCount} of {totalCount} revealed</span>
          </div>
          <div class="study-bar-actions">
            {#if revealedCount < totalCount}
              <button type="button" class="study-action-btn" onclick={revealAll}>
                <Eye size={11} />
                <span>Reveal All</span>
              </button>
            {:else}
              <button type="button" class="study-action-btn" onclick={hideAll}>
                <RotateCcw size={11} />
                <span>Reset All</span>
              </button>
            {/if}
          </div>
        </div>
      {/if}
    </header>

    <!-- Main Cornell 2-Column Grid -->
    <div class="cornell-grid">
      <!-- Column Labels Header -->
      <div class="grid-labels">
        <div class="cue-label">
          <span>CUES · QUESTIONS · KEYWORDS</span>
        </div>
        <div class="margin-divider-head" aria-hidden="true"></div>
        <div class="notes-label">
          <span>LECTURE NOTES · DEFINITIONS & SUPPORTING DETAILS</span>
        </div>
      </div>

      <!-- Stream of Cornell Items -->
      <div class="items-stream">
        {#each doc.current.items as item, index (item.$id)}
          {@const isRevealed = revealedItems[item.$id]}
          <div class="cornell-entry">
            <!-- Left: Cues Column (Questions & Prompts) -->
            <div class="cue-cell">
              <span class="cue-prefix" aria-hidden="true">Q{index + 1}</span>
              <textarea
                class="cue-textarea"
                rows={1}
                use:bindText={doc.at(item).cue}
                use:autosize
                placeholder="Key question or prompt…"
                aria-label={`Cue ${index + 1}`}
              ></textarea>
              {#if studyMode}
                <button
                  type="button"
                  class="cue-reveal-btn"
                  class:is-revealed={isRevealed}
                  data-slop-export="hide"
                  onclick={() => toggleReveal(item.$id)}
                >
                  {#if isRevealed}
                    <EyeOff size={11} />
                    <span>Hide</span>
                  {:else}
                    <Eye size={11} />
                    <span>Check</span>
                  {/if}
                </button>
              {/if}
            </div>

            <!-- Authentic Carnelian Red Vertical Margin Rule -->
            <div class="margin-rule" aria-hidden="true"></div>

            <!-- Right: Notes Column -->
            <div class="notes-cell">
              {#if studyMode && !isRevealed}
                <!-- Tactile Active Recall Study Mask -->
                <button
                  type="button"
                  class="study-mask-card"
                  data-slop-export="hide"
                  onclick={() => toggleReveal(item.$id)}
                  title="Click to reveal note"
                >
                  <Eye size={14} />
                  <span>Tap or space to reveal notes</span>
                </button>
              {/if}

              <div class="notes-content-wrap" class:masked-notes={studyMode && !isRevealed}>
                <textarea
                  class="notes-textarea"
                  rows={2}
                  use:bindText={doc.at(item).notes}
                  use:autosize
                  placeholder="- Enter detailed notes, definitions, formulas…"
                  aria-label={`Notes for ${item.cue || `Item ${index + 1}`}`}
                ></textarea>
              </div>

              <button
                type="button"
                class="delete-btn"
                data-slop-export="hide"
                onclick={() => removeItem(item.$id)}
                aria-label={`Delete item ${index + 1}`}
                title="Delete note block"
              >
                <Trash2 size={12} />
              </button>
            </div>
          </div>
        {/each}
      </div>

      <!-- Add Row Bar -->
      <div class="add-entry-bar" data-slop-export="hide">
        <button type="button" class="add-entry-btn" onclick={addItem}>
          <Plus size={13} />
          <span>Add Cornell Note Block</span>
        </button>
      </div>
    </div>

    <!-- Bottom: Summary & Synthesis Box -->
    <footer class="summary-box">
      <div class="summary-title-row">
        <span class="summary-kicker">SUMMARY & SYNTHESIS</span>
        <span class="summary-sub">Briefly capture the big picture in 2–3 sentences</span>
      </div>
      <textarea
        class="summary-textarea"
        rows={2}
        use:bindText={doc.fields.summary}
        use:autosize
        placeholder="Synthesize the core takeaways of this lecture in your own words…"
        aria-label="Summary and Synthesis"
      ></textarea>
    </footer>
  </div>
</main>

{#snippet exportView()}
  <main class="cornell-page" class:theme-dark={currentTheme === "dark"}>
    <div class="notebook-sheet">
      <header class="sheet-header">
        <div class="header-top-bar">
          <div class="course-meta">
            <div class="course-badge"><span class="course-input">{doc.current.course}</span></div>
            <span class="meta-sep">·</span>
            <div class="meta-item"><span class="meta-label">INSTRUCTOR</span><span class="meta-input">{doc.current.lecturer}</span></div>
            <span class="meta-sep">·</span>
            <strong class="date-text">{doc.current.date}</strong>
          </div>
        </div>
        <div class="topic-row"><h1 class="topic-input">{doc.current.topic}</h1></div>
      </header>
      <div class="cornell-grid">
        <div class="items-stream">
          {#each doc.current.items as item, index (item.$id)}
            <div class="cornell-entry">
              <div class="cue-cell"><span class="cue-prefix">Q{index + 1}</span><p class="cue-textarea">{item.cue}</p></div>
              <div class="margin-rule"></div>
              <div class="notes-cell"><p class="notes-textarea">{item.notes}</p></div>
            </div>
          {/each}
        </div>
      </div>
      <footer class="summary-box"><p class="summary-textarea">{doc.current.summary}</p></footer>
    </div>
  </main>
{/snippet}

{#snippet icon()}
<section class="cornell-render cornell-icon" data-slop-render="icon" aria-hidden="true">
  <div class="icon-sheet">
    <div class="icon-head">
      <div class="icon-topic-line"></div>
      <div class="icon-course-pill"></div>
    </div>
    <div class="icon-body">
      <div class="icon-cue-col">
        <span class="icon-cue-bar"></span>
        <span class="icon-cue-bar short"></span>
        <span class="icon-cue-bar"></span>
        <span class="icon-cue-bar short"></span>
      </div>
      <div class="icon-red-rule"></div>
      <div class="icon-notes-col">
        <span class="icon-note-bar"></span>
        <span class="icon-note-bar"></span>
        <span class="icon-note-bar mid"></span>
        <span class="icon-note-bar"></span>
        <span class="icon-note-bar short"></span>
        <span class="icon-note-bar"></span>
        <span class="icon-note-bar mid"></span>
      </div>
    </div>
    <div class="icon-summary-box">
      <span class="icon-summary-head"></span>
      <span class="icon-summary-line"></span>
      <span class="icon-summary-line mid"></span>
    </div>
  </div>
</section>
{/snippet}
</Slop>
