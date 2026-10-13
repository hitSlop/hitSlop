<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { formatDateStr, getTypeColor, semesterView } from "./shared";

  // The capture cannot see the editor's course filter, so it lists every milestone.
  const view = $derived(semesterView(doc.current));
</script>

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
      {#each (preview ? (view.months).slice(0, 3) : view.months) as month}
        <div class="month-column">
          <div class="month-header"><span class="month-name">{month.name}</span><span class="month-year">{month.year}</span></div>
          <div class="month-days-grid">
            {#each Array.from({ length: month.days }, (_, i) => i + 1) as dayNum}
              {@const dateStr = formatDateStr(month.year, month.month, dayNum)}
              {@const dayMilestones = view.on(dateStr)}
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
          {#each (preview ? (view.agenda).slice(0, 6) : view.agenda) as ms (ms.$id)}
            <tr class="agenda-row">
              <td class="date-cell">{ms.date}</td>
              <td><span class="course-badge" style="background-color: {view.courseColor(ms.courseCode)};">{ms.courseCode}</span></td>
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
