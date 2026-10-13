<script lang="ts">
  import doc from "./schema";
  const complete = $derived(doc.current.tasks.filter(task => task.done).length);
</script>

<main class="slop-paper" style="background:var(--slop-surface)">
  <div class="slop-eyebrow">Little checklist</div>
  <h1 class="slop-title">{doc.current.title || "Your checklist"}</h1>
  <p class="slop-summary">{complete} of {doc.current.tasks.length} done.</p>
  <ul class="slop-list">
    {#each doc.current.tasks.slice(0, 5) as task (task.$id)}
      <li class="slop-row" data-done={task.done}>
        <span class="slop-check" data-state={task.done ? "checked" : "unchecked"}>{task.done ? "✓" : ""}</span>
        <span class="slop-text">{task.text}</span>
      </li>
    {/each}
  </ul>
</main>
