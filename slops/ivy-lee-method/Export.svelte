<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { docketView, SLOTS } from "./tasks";
  let { snapshot }: CaptureProps<typeof schema> = $props();

  const { completed, percent } = $derived(docketView(snapshot));
</script>

<main class="ledger-container">
  <header class="ledger-header">
    <div class="brand-row">
      <span class="brand-mark">NO. 1918</span>
      <span class="brand-rule">RULE: STRICT SINGLE-TASKING</span>
    </div>
    <div class="title-row">
      <h1 class="ledger-title">Ivy Lee Method</h1>
      <p class="ledger-date">{snapshot.date}</p>
    </div>
    <div class="progress-section">
      <div class="progress-labels">
        <span class="progress-text">{completed} of {SLOTS} finished</span>
        <span class="completion-pct">{percent}%</span>
      </div>
    </div>
  </header>
  <section class="slots-section" aria-label="Six daily tasks">
    {#each snapshot.tasks as task, index (task.$id)}
      <article class="task-row" class:is-done={task.done}>
        <div class="slot-badge"><span class="slot-num">0{index + 1}</span></div>
        <span class="task-input">{task.text || `Task ${index + 1}`}</span>
      </article>
    {/each}
  </section>
  {#if snapshot.notes.trim()}
    <footer class="ledger-footer"><p class="ledger-notes">{snapshot.notes}</p></footer>
  {/if}
</main>
