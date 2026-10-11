<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { bindText } from "hitslop/svelte";
  import Check from "@lucide/svelte/icons/check";
  import Clock from "@lucide/svelte/icons/clock";
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import { Checkbox } from "bits-ui";
  import doc from "./schema";
  import { adjustFocus, planView, setTaskDone, toggleHour } from "./plan";

  type PlanTask = (typeof doc.current.shortTasks)[number];

  const view = $derived(planView(doc.current));

  async function nudgeFocus(delta: number) {
    await doc.change(tx => adjustFocus(tx, doc.current, delta));
  }

  async function pickHour(hour: number) {
    await doc.change(tx => toggleHour(tx, doc.current, hour));
  }
</script>

{#snippet taskRow(task: PlanTask, index: number)}
  <li class="task-item" class:is-done={task.done}>
    <Checkbox.Root
      checked={task.done}
      onCheckedChange={(checked) => doc.change(tx => setTaskDone(tx, doc.current, task, checked === true))}
      class="check-btn"
      aria-label={task.done ? "Mark task incomplete" : "Mark task complete"}
    >
      {#snippet children({ checked })}
        {#if checked}<Check size={12} strokeWidth={3} />{/if}
      {/snippet}
    </Checkbox.Root>
    <span class="item-index">#{index + 1}</span>
    <input type="text" class="task-input" placeholder="Task #{index + 1}..." use:bindText={doc.at(task).text} />
  </li>
{/snippet}

{#snippet taskLine(task: PlanTask, index: number)}
  <li class="task-item" class:is-done={task.done}>
    <span class="item-index">#{index + 1}</span>
    <span class="task-input">{task.text}</span>
  </li>
{/snippet}

  <main class="plan-container">
    <header class="sheet-header">
      <div class="header-top">
        <span class="philosophy-badge">OLIVER BURKEMAN • FINITE DAY DOCKET</span>
        <div class="stats-pills" data-slop-export="hide">
          <span class="pill">{view.hours}h {view.mins}m / 3h Focus</span>
          <span class="pill">{view.totalDone}/6 Tasks Done</span>
        </div>
      </div>
      <div class="header-main">
        <h1 class="plan-title">The 3-3-3 Plan</h1>
        <input type="text" class="plan-date" bind:value={doc.fields.date.value} aria-label="Plan Date" />
      </div>
    </header>

    <section class="section-card deep-work-card">
      <div class="card-header">
        <div class="card-label">
          <span class="num-badge deep-num">3</span>
          <div class="label-copy">
            <h2 class="card-title">Hours of Deep Work</h2>
            <span class="card-subtitle">One core, challenging project with zero distraction</span>
          </div>
        </div>
        <div class="time-controls" data-slop-export="hide">
          <button type="button" class="stepper-btn" onclick={() => nudgeFocus(-15)} title="Subtract 15 min" aria-label="Subtract 15 min">
            <Minus size={12} />
          </button>
          <span class="time-readout">{view.clock}</span>
          <button type="button" class="stepper-btn" onclick={() => nudgeFocus(15)} title="Add 15 min" aria-label="Add 15 min">
            <Plus size={12} />
          </button>
        </div>
      </div>

      <div class="hour-segments">
        {#each view.hourBlocks as block (block.hour)}
          <button
            type="button"
            class="hour-block"
            class:is-filled={block.filled}
            class:is-partial={block.partial}
            onclick={() => pickHour(block.hour)}
            title="Toggle Hour {block.hour}"
          >
            <Clock size={11} />
            <span>Hour 0{block.hour}</span>
            {#if block.filled}<span class="block-check"><Check size={10} strokeWidth={3} /></span>{/if}
          </button>
        {/each}
      </div>

      <input type="text" class="project-input" placeholder="Name the single deep-work project..." use:bindText={doc.fields.deepWork.project} aria-label="Deep work project title" />
      <textarea class="project-notes" placeholder="Session goals, scope boundaries, or breakthrough notes..." use:bindText={doc.fields.deepWork.notes} rows={2}></textarea>
    </section>

    <section class="section-card short-card">
      <div class="card-header">
        <div class="card-label">
          <span class="num-badge short-num">3</span>
          <div class="label-copy">
            <h2 class="card-title">Urgent / Shorter Tasks</h2>
            <span class="card-subtitle">Tasks you have been avoiding or pressing obligations</span>
          </div>
        </div>
        <span class="card-count">{view.shortDone}/3</span>
      </div>
      <ul class="task-list">
        {#each doc.current.shortTasks as task, index (task.$id)}
          {@render taskRow(task, index)}
        {/each}
      </ul>
    </section>

    <section class="section-card maint-card">
      <div class="card-header">
        <div class="card-label">
          <span class="num-badge maint-num">3</span>
          <div class="label-copy">
            <h2 class="card-title">Maintenance Activities</h2>
            <span class="card-subtitle">Routines, admin, health, and household loops to keep life running</span>
          </div>
        </div>
        <span class="card-count">{view.maintDone}/3</span>
      </div>
      <ul class="task-list">
        {#each doc.current.maintenance as task, index (task.$id)}
          {@render taskRow(task, index)}
        {/each}
      </ul>
    </section>
  </main>
