<script lang="ts">
  import type { Value } from "hitslop";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { planView } from "./plan";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  type PlanTask = Value<typeof schema.descriptor>["shortTasks"][number];

  const view = $derived(planView(snapshot));
</script>

<main class="plan-container">
  <header class="sheet-header">
    <div class="header-top">
      <span class="philosophy-badge">OLIVER BURKEMAN • FINITE DAY DOCKET</span>
    </div>
    <div class="header-main">
      <h1 class="plan-title">The 3-3-3 Plan</h1>
      <p class="plan-date">{snapshot.date}</p>
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
    <p class="project-input">{snapshot.deepWork.project}</p>
    {#if snapshot.deepWork.notes.trim()}<p class="project-notes">{snapshot.deepWork.notes}</p>{/if}
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
      {#each snapshot.shortTasks as task, index (task.$id)}{@render taskLine(task, index)}{/each}
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
      {#each snapshot.maintenance as task, index (task.$id)}{@render taskLine(task, index)}{/each}
    </ul>
  </section>
</main>

{#snippet taskLine(task: PlanTask, index: number)}
  <li class="task-item" class:is-done={task.done}>
    <span class="item-index">#{index + 1}</span>
    <span class="task-input">{task.text}</span>
  </li>
{/snippet}
