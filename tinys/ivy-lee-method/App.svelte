<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "@hitslop/tinys/svelte";
  import Check from "@lucide/svelte/icons/check";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Zap from "@lucide/svelte/icons/zap";
  import { Checkbox, Progress } from "bits-ui";
  import schema from "./schema";
  import { docketView, moveTask, rollover, setDone, SLOTS } from "./tasks";

  const doc = getDocument(schema);

  function tomorrowStr(): string {
    const d = new Date();
    d.setDate(d.getDate() + 1);
    return d.toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
  }

  const { completed, percent, activeIndex } = $derived(docketView(doc.current));

  async function move(id: string, direction: "up" | "down") {
    await doc.change((tx) => moveTask(tx, doc.current, id, direction));
  }

  async function rollOver() {
    await doc.change((tx) => rollover(tx, doc.current, tomorrowStr()));
  }
</script>

  <main class="ledger-container">
    <header class="ledger-header">
      <div class="brand-row">
        <span class="brand-mark">NO. 1918</span>
        <span class="brand-rule">RULE: STRICT SINGLE-TASKING</span>
      </div>
      <div class="title-row">
        <h1 class="ledger-title">Ivy Lee Method</h1>
        <input type="text" class="ledger-date" bind:value={doc.fields.date.value} aria-label="Docket Date" />
      </div>

      <div class="progress-section">
        <div class="progress-labels">
          <span class="progress-text">{completed} of {SLOTS} finished</span>
          <span class="completion-pct">{percent}%</span>
        </div>
        <Progress.Root value={completed} max={SLOTS} class="progress-track" aria-label="Ivy Lee daily progress">
          {#each { length: SLOTS } as _, i}
            <div class="progress-step" class:step-filled={i < completed}></div>
          {/each}
        </Progress.Root>
      </div>
    </header>

    <section class="slots-section" aria-label="Six daily tasks">
      {#each doc.current.tasks as task, index (task.$id)}
        {@const isActive = index === activeIndex}
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
            onCheckedChange={async (checked) => {
              await doc.change((tx) => setDone(tx, task.$id, checked === true));
            }}
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

          <div class="slot-actions" data-tiny-export="hide">
            <button type="button" class="order-btn" disabled={index === 0} onclick={() => move(task.$id, "up")} aria-label="Move up">
              <ArrowUp size={12} />
            </button>
            <button type="button" class="order-btn" disabled={index === SLOTS - 1} onclick={() => move(task.$id, "down")} aria-label="Move down">
              <ArrowDown size={12} />
            </button>
          </div>
        </article>
      {/each}
    </section>

    <footer class="ledger-footer">
      <textarea class="ledger-notes" placeholder="Reflections or end-of-day notes..." use:bindText={doc.fields.notes} rows={2}></textarea>
      <div class="footer-actions" data-tiny-export="hide">
        <button type="button" class="rollover-btn" onclick={rollOver} title="Carry over incomplete tasks to tomorrow's fresh docket">
          <RotateCcw size={13} />
          <span>Roll Over Unfinished to Tomorrow</span>
        </button>
      </div>
    </footer>
  </main>
