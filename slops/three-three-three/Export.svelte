<script lang="ts">
  import doc from "./schema";
  import { planView } from "./plan";

  type PlanTask = (typeof doc.current.shortTasks)[number];

  const view = $derived(planView(doc.current));
</script>

<main class="plan-container">
  <header class="sheet-header">
    <div class="header-top">
      <span class="philosophy-badge">OLIVER BURKEMAN • FINITE DAY DOCKET</span>
    </div>
    <div class="header-main">
      <h1 class="plan-title">The 3-3-3 Plan</h1>
      <p class="plan-date">{doc.current.date}</p>
    </div>
  </header>
  <section class="section-card deep-work-card">
    <div class="card-header">
      <div class="card-label">
        <span class="num-badge deep-num">3</span>
        <div class="label-copy">
          <h2 class="card-title">Hours of Deep Work</h2>
          <span class="card-subtitle">{view.clock} logged</span>
        </div>
      </div>
    </div>
    <p class="project-input">{doc.current.deepWork.project}</p>
    {#if doc.current.deepWork.notes.trim()}<p class="project-notes">{doc.current.deepWork.notes}</p>{/if}
  </section>
  <section class="section-card short-card">
    <div class="card-header">
      <div class="card-label">
        <span class="num-badge short-num">3</span>
        <div class="label-copy"><h2 class="card-title">Urgent / Shorter Tasks</h2></div>
      </div>
      <span class="card-count">{view.shortDone}/3</span>
    </div>
    <ul class="task-list">
      {#each doc.current.shortTasks as task, index (task.$id)}{@render taskLine(task, index)}{/each}
    </ul>
  </section>
  <section class="section-card maint-card">
    <div class="card-header">
      <div class="card-label">
        <span class="num-badge maint-num">3</span>
        <div class="label-copy"><h2 class="card-title">Maintenance Activities</h2></div>
      </div>
      <span class="card-count">{view.maintDone}/3</span>
    </div>
    <ul class="task-list">
      {#each doc.current.maintenance as task, index (task.$id)}{@render taskLine(task, index)}{/each}
    </ul>
  </section>
</main>

{#snippet taskLine(task: PlanTask, index: number)}
  <li class="task-item" class:is-done={task.done}>
    <span class="item-index">#{index + 1}</span>
    <span class="task-input">{task.text}</span>
  </li>
{/snippet}
