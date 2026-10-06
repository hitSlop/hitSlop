<script lang="ts">
  import doc from "./schema";

  const completedCount = $derived(doc.current.tasks.filter((task) => task.done).length);
</script>

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
