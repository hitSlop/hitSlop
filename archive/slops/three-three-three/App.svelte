<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import Check from "@lucide/svelte/icons/check";
  import Clock from "@lucide/svelte/icons/clock";
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import { Checkbox } from "bits-ui";
  import schema, { type PlanTask } from "./schema";

  const doc = useDocument(schema);

  const hoursLogged = $derived(Math.floor(doc.current.deepWork.minutes / 60));
  const minsRemainder = $derived(doc.current.deepWork.minutes % 60);
  const shortDoneCount = $derived(doc.current.shortTasks.filter((task) => task.done).length);
  const maintDoneCount = $derived(doc.current.maintenance.filter((task) => task.done).length);
  const totalTasksDone = $derived(shortDoneCount + maintDoneCount);

  function adjustMinutes(delta: number) {
    const next = Math.max(0, Math.min(180, doc.current.deepWork.minutes + delta));
    doc.fields.deepWork.minutes.set(next);
  }

  function setHourDirect(hour: number) {
    const targetMins = hour * 60;
    const current = doc.current.deepWork.minutes;
    doc.fields.deepWork.minutes.set(current === targetMins ? (hour - 1) * 60 : targetMins);
  }
</script>

{#snippet taskRow(task: PlanTask, index: number)}
  <li class="task-item" class:is-done={task.done}>
    <Checkbox.Root
      checked={task.done}
      onCheckedChange={(checked) => doc.at(task).done.set(checked === true)}
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

<Slop>
  <main class="plan-container">
    <header class="sheet-header">
      <div class="header-top">
        <span class="philosophy-badge">OLIVER BURKEMAN • FINITE DAY DOCKET</span>
        <div class="stats-pills" data-slop-export="hide">
          <span class="pill">{hoursLogged}h {minsRemainder}m / 3h Focus</span>
          <span class="pill">{totalTasksDone}/6 Tasks Done</span>
        </div>
      </div>
      <div class="header-main">
        <h1 class="plan-title">The 3-3-3 Plan</h1>
        <input type="text" class="plan-date" use:bindValue={doc.fields.date} aria-label="Plan Date" />
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
          <button type="button" class="stepper-btn" onclick={() => adjustMinutes(-15)} title="Subtract 15 min" aria-label="Subtract 15 min">
            <Minus size={12} />
          </button>
          <span class="time-readout">{hoursLogged}h {String(minsRemainder).padStart(2, "0")}m</span>
          <button type="button" class="stepper-btn" onclick={() => adjustMinutes(15)} title="Add 15 min" aria-label="Add 15 min">
            <Plus size={12} />
          </button>
        </div>
      </div>

      <div class="hour-segments">
        {#each [1, 2, 3] as hour}
          {@const filled = doc.current.deepWork.minutes >= hour * 60}
          {@const partial = !filled && doc.current.deepWork.minutes > (hour - 1) * 60}
          <button
            type="button"
            class="hour-block"
            class:is-filled={filled}
            class:is-partial={partial}
            onclick={() => setHourDirect(hour)}
            title="Toggle Hour {hour}"
          >
            <Clock size={11} />
            <span>Hour 0{hour}</span>
            {#if filled}<span class="block-check"><Check size={10} strokeWidth={3} /></span>{/if}
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
        <span class="card-count">{shortDoneCount}/3</span>
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
        <span class="card-count">{maintDoneCount}/3</span>
      </div>
      <ul class="task-list">
        {#each doc.current.maintenance as task, index (task.$id)}
          {@render taskRow(task, index)}
        {/each}
      </ul>
    </section>
  </main>

  {#snippet exportView()}
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
              <span class="card-subtitle">{hoursLogged}h {String(minsRemainder).padStart(2, "0")}m logged</span>
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
          <span class="card-count">{shortDoneCount}/3</span>
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
          <span class="card-count">{maintDoneCount}/3</span>
        </div>
        <ul class="task-list">
          {#each doc.current.maintenance as task, index (task.$id)}{@render taskLine(task, index)}{/each}
        </ul>
      </section>
    </main>
  {/snippet}

  {#snippet icon()}
    <section class="ttt-render ttt-icon" data-slop-render="icon" aria-hidden="true">
      <article class="icon-page">
        <div class="icon-bands">
          <div class="icon-band-deep">
            <span class="icon-dot-hour"></span>
            <span class="icon-dot-hour"></span>
            <span class="icon-dot-hour"></span>
          </div>
          <div class="icon-band-sub">
            <span class="icon-pip"></span>
            <span class="icon-pip"></span>
            <span class="icon-pip"></span>
          </div>
          <div class="icon-band-sub">
            <span class="icon-pip"></span>
            <span class="icon-pip"></span>
            <span class="icon-pip"></span>
          </div>
        </div>
      </article>
    </section>
  {/snippet}
</Slop>
