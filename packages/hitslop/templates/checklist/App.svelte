<script lang="ts">
  import { Checkbox } from "bits-ui";
  import { EditableText, draft } from "hitslop/svelte";
  import doc from "./schema";
  import { addTask } from "./commands";

  const { title, tasks } = doc.fields;
  const complete = $derived(doc.current.tasks.filter(task => task.done).length);
  // The composer's text; it clears once the task is added. A refusal such as
  // "Enter a task." appears in the window's notice region.
  const newTask = draft(text => addTask({ text }));
  let entry = $state<HTMLInputElement>();
</script>

<main class="slop-paper">
  <div class="slop-eyebrow">Little checklist</div>
  <EditableText class="slop-title" field={title} label="List title" placeholder="Name your list" />
  <p class="slop-summary">{complete} of {doc.current.tasks.length} done. One thing at a time.</p>
  {#if !doc.current.tasks.length}<p class="slop-summary">A little breathing room. Add your first task below.</p>{/if}
  <ul class="slop-list">
    {#each doc.current.tasks as task, index (task.$id)}
      {@const row = doc.at(task)}
      <li class="slop-row" data-done={task.done}>
        <Checkbox.Root class="slop-check" aria-label={`Complete ${task.text}`} bind:checked={row.done.value}>
          {#if task.done}<span aria-hidden="true">✓</span>{/if}
        </Checkbox.Root>
        <EditableText class="slop-text" field={row.text} label="Task text" placeholder="Untitled task" onenter={() => entry?.focus()} />
        <div class="slop-actions" data-slop-export="hide">
          <button class="slop-small" aria-label="Move task up" disabled={index === 0} onclick={() => tasks.move(task.$id, { before: doc.current.tasks[index - 1]!.$id })}>↑</button>
          <button class="slop-small" aria-label="Move task down" disabled={index === doc.current.tasks.length - 1} onclick={() => tasks.move(task.$id, { after: doc.current.tasks[index + 1]!.$id })}>↓</button>
          <button class="slop-small" aria-label="Delete task" onclick={() => tasks.remove(task.$id)}>×</button>
        </div>
      </li>
    {/each}
  </ul>
  <form class="slop-add" onsubmit={newTask.submit} data-slop-export="hide">
    <input class="slop-entry" aria-label="New task" placeholder="Something to do…" bind:this={entry} bind:value={newTask.value} />
    <button class="slop-button" disabled={!newTask.ready}>Add</button>
  </form>
  <footer class="slop-footer">
    <span>Your list, at your pace.</span>
  </footer>
</main>
