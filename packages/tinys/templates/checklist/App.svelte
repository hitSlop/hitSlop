<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { Checkbox } from "bits-ui";
  import { EditableText, draft, getDocument } from "@hitslop/tinys/svelte";
  import schema, { addTask } from "./schema";

  const doc = getDocument(schema);
  const { title, tasks } = doc.fields;
  const complete = $derived(doc.current.tasks.filter(task => task.done).length);
  // The composer's text; it clears once the task is added. A refusal such as
  // "Enter a task." appears in the window's notice region.
  const newTask = draft(text => doc.change(tx => addTask(tx, text)));
  let entry = $state<HTMLInputElement>();
</script>

<main class="tiny-paper">
  <div class="tiny-eyebrow">Little checklist</div>
  <EditableText class="tiny-title" field={title} label="List title" placeholder="Name your list" />
  <p class="tiny-summary">{complete} of {doc.current.tasks.length} done. One thing at a time.</p>
  {#if !doc.current.tasks.length}<p class="tiny-summary">A little breathing room. Add your first task below.</p>{/if}
  <ul class="tiny-list">
    {#each doc.current.tasks as task, index (task.$id)}
      {@const row = doc.at(task)}
      <li class="tiny-row" data-done={task.done}>
        <Checkbox.Root class="tiny-check" aria-label={`Complete ${task.text}`} bind:checked={row.done.value}>
          {#if task.done}<span aria-hidden="true">✓</span>{/if}
        </Checkbox.Root>
        <EditableText class="tiny-text" field={row.text} label="Task text" placeholder="Untitled task" onenter={() => entry?.focus()} />
        <div class="tiny-actions" data-tiny-export="hide">
          <button class="tiny-small" aria-label="Move task up" disabled={index === 0} onclick={() => tasks.move(task, { before: doc.current.tasks[index - 1]! })}>↑</button>
          <button class="tiny-small" aria-label="Move task down" disabled={index === doc.current.tasks.length - 1} onclick={() => tasks.move(task, { after: doc.current.tasks[index + 1]! })}>↓</button>
          <button class="tiny-small" aria-label="Delete task" onclick={() => tasks.remove(task)}>×</button>
        </div>
      </li>
    {/each}
  </ul>
  <form class="tiny-add" onsubmit={newTask.submit} data-tiny-export="hide">
    <input class="tiny-entry" aria-label="New task" placeholder="Something to do…" bind:this={entry} bind:value={newTask.value} />
    <button class="tiny-button" disabled={!newTask.ready}>Add</button>
  </form>
  <footer class="tiny-footer">
    <span>Your list, at your pace.</span>
  </footer>
</main>
