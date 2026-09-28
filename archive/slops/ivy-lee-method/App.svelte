<script lang="ts">
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import Check from "@lucide/svelte/icons/check";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Zap from "@lucide/svelte/icons/zap";
  import { Checkbox, Progress } from "bits-ui";
  import schema from "./schema";

  function tomorrowStr(): string {
    const d = new Date();
    d.setDate(d.getDate() + 1);
    return d.toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
  }

  const doc = useDocument(schema);

  const completedCount = $derived(doc.current.tasks.filter((task) => task.done).length);
  const activeTaskIndex = $derived(doc.current.tasks.findIndex((task) => !task.done));

  function moveUp(index: number) {
    const tasks = doc.current.tasks;
    const task = tasks[index];
    const previous = tasks[index - 1];
    if (!task || !previous) return;
    doc.fields.tasks.move(task.$id, { before: previous.$id });
  }

  function moveDown(index: number) {
    const tasks = doc.current.tasks;
    const task = tasks[index];
    const next = tasks[index + 1];
    if (!task || !next) return;
    doc.fields.tasks.move(task.$id, { after: next.$id });
  }

  function rollover() {
    const tasks = doc.current.tasks;
    const incomplete = tasks.filter((task) => !task.done);
    doc.change((tx) => {
      for (let i = 0; i < tasks.length; i++) {
        const row = tasks[i];
        if (!row) continue;
        tx.at(row).text.replace(incomplete[i]?.text ?? "");
        tx.at(row).done.set(false);
      }
      tx.fields.date.set(tomorrowStr());
    });
  }
</script>

<Slop>
  <main class="ledger-container">
    <header class="ledger-header">
      <div class="brand-row">
        <span class="brand-mark">NO. 1918</span>
        <span class="brand-rule">RULE: STRICT SINGLE-TASKING</span>
      </div>
      <div class="title-row">
        <h1 class="ledger-title">Ivy Lee Method</h1>
        <input type="text" class="ledger-date" use:bindValue={doc.fields.date} aria-label="Docket Date" />
      </div>

      <div class="progress-section">
        <div class="progress-labels">
          <span class="progress-text">{completedCount} of 6 finished</span>
          <span class="completion-pct">{Math.round((completedCount / 6) * 100)}%</span>
        </div>
        <Progress.Root value={completedCount} max={6} class="progress-track" aria-label="Ivy Lee daily progress">
          {#each { length: 6 } as _, i}
            <div class="progress-step" class:step-filled={i < completedCount}></div>
          {/each}
        </Progress.Root>
      </div>
    </header>

    <section class="slots-section" aria-label="Six daily tasks">
      {#each doc.current.tasks as task, index (task.$id)}
        {@const isActive = index === activeTaskIndex}
        <article class="task-row" class:is-active={isActive} class:is-done={task.done}>
          <div class="slot-badge">
            <span class="slot-num">0{index + 1}</span>
            {#if isActive}
              <span class="active-indicator" title="Current Single Focus">
                <Zap size={11} strokeWidth={2.5} />
              </span>
            {/if}
          </div>

          <Checkbox.Root
            checked={task.done}
            onCheckedChange={(checked) => doc.at(task).done.set(checked === true)}
            class="toggle-btn"
            aria-label={task.done ? `Mark slot ${index + 1} incomplete` : `Mark slot ${index + 1} complete`}
          >
            {#snippet children({ checked })}
              {#if checked}
                <Check size={13} strokeWidth={3} />
              {/if}
            {/snippet}
          </Checkbox.Root>

          <input type="text" class="task-input" placeholder="Task #{index + 1}..." use:bindText={doc.at(task).text} />

          <div class="slot-actions" data-slop-export="hide">
            <button type="button" class="order-btn" disabled={index === 0} onclick={() => moveUp(index)} aria-label="Move up">
              <ArrowUp size={12} />
            </button>
            <button type="button" class="order-btn" disabled={index === 5} onclick={() => moveDown(index)} aria-label="Move down">
              <ArrowDown size={12} />
            </button>
          </div>
        </article>
      {/each}
    </section>

    <footer class="ledger-footer">
      <textarea class="ledger-notes" placeholder="Reflections or end-of-day notes..." use:bindText={doc.fields.notes} rows={2}></textarea>
      <div class="footer-actions" data-slop-export="hide">
        <button type="button" class="rollover-btn" onclick={rollover} title="Carry over incomplete tasks to tomorrow's fresh docket">
          <RotateCcw size={13} />
          <span>Roll Over Unfinished to Tomorrow</span>
        </button>
      </div>
    </footer>
  </main>

  {#snippet exportView()}
    <main class="ledger-container">
      <header class="ledger-header">
        <div class="brand-row">
          <span class="brand-mark">NO. 1918</span>
          <span class="brand-rule">RULE: STRICT SINGLE-TASKING</span>
        </div>
        <div class="title-row">
          <h1 class="ledger-title">Ivy Lee Method</h1>
          <p class="ledger-date">{doc.current.date}</p>
        </div>
        <div class="progress-section">
          <div class="progress-labels">
            <span class="progress-text">{completedCount} of 6 finished</span>
            <span class="completion-pct">{Math.round((completedCount / 6) * 100)}%</span>
          </div>
        </div>
      </header>
      <section class="slots-section" aria-label="Six daily tasks">
        {#each doc.current.tasks as task, index (task.$id)}
          <article class="task-row" class:is-done={task.done}>
            <div class="slot-badge"><span class="slot-num">0{index + 1}</span></div>
            <span class="task-input">{task.text || `Task ${index + 1}`}</span>
          </article>
        {/each}
      </section>
      {#if doc.current.notes.trim()}
        <footer class="ledger-footer"><p class="ledger-notes">{doc.current.notes}</p></footer>
      {/if}
    </main>
  {/snippet}

  {#snippet icon()}
    <section class="ivy-render ivy-icon" data-slop-render="icon" aria-hidden="true">
      <article class="icon-page">
        <div class="icon-strip">
          <span class="icon-slot icon-active"></span>
          <span class="icon-slot"></span>
          <span class="icon-slot"></span>
          <span class="icon-slot"></span>
          <span class="icon-slot"></span>
          <span class="icon-slot"></span>
        </div>
      </article>
    </section>
  {/snippet}
</Slop>
