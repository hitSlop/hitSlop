<script lang="ts">
  import { tick } from "svelte";
  import { Checkbox } from "bits-ui";
  import { bindText } from "hitslop/svelte";
  import doc from "./schema";
  import { addTask } from "./commands";

  const { title, tasks } = doc.fields;
  const taskInputs = new Map<string, HTMLInputElement>();
  function taskInput(node: HTMLInputElement, id: string) {
    taskInputs.set(id, node);
    return { destroy() { taskInputs.delete(id); } };
  }
  let newTask = $state("");
  const complete = $derived(doc.current.tasks.filter(task => task.done).length);
  let adding = $state(false);
  let addError = $state("");
  async function add() {
    if (!newTask.trim() || adding) return;
    const submitted = newTask;
    adding = true;
    addError = "";
    try {
      const { id } = await addTask({ text: submitted });
      if (newTask === submitted) newTask = "";
      await tick();
      taskInputs.get(id)?.focus();
    } catch (error) {
      addError = error instanceof Error ? error.message : "Could not add the task. Try again.";
    } finally { adding = false; }
  }
</script>

<main class="slop-paper">
  <div class="slop-eyebrow">Little checklist</div>
  <input class="slop-title" aria-label="List title" use:bindText={title} />
  <p class="slop-summary">{complete} of {doc.current.tasks.length} done. One thing at a time.</p>
  {#if !doc.current.tasks.length}<p class="slop-summary">A little breathing room. Add your first task below.</p>{/if}
  <ul class="slop-list">
    {#each doc.current.tasks as task, index (task.$id)}
      {@const row = doc.at(task)}
      <li class="slop-row" data-task-id={task.$id}>
        <Checkbox.Root class="slop-check" aria-label={`Complete ${task.text}`} bind:checked={row.done.value}>
          {#if task.done}<span aria-hidden="true">✓</span>{/if}
        </Checkbox.Root>
        <input class="slop-text" data-done={task.done} aria-label="Task text" use:bindText={row.text} use:taskInput={task.$id} />
        <div class="slop-actions" data-slop-export="hide">
          <button class="slop-small" aria-label="Move task up" disabled={index === 0} onclick={() => tasks.move(task.$id, { before: doc.current.tasks[index - 1]!.$id })}>↑</button>
          <button class="slop-small" aria-label="Move task down" disabled={index === doc.current.tasks.length - 1} onclick={() => tasks.move(task.$id, { after: doc.current.tasks[index + 1]!.$id })}>↓</button>
          <button class="slop-small" aria-label="Delete task" onclick={() => tasks.remove(task.$id)}>×</button>
        </div>
      </li>
    {/each}
  </ul>
  <form class="slop-add" onsubmit={(event) => { event.preventDefault(); return add(); }} data-slop-export="hide">
    <input class="slop-entry" aria-label="New task" placeholder="Something to do…" bind:value={newTask} />
    <button class="slop-button" disabled={adding || !newTask.trim()}>Add</button>
  </form>
  {#if addError}<p role="alert" class="slop-summary" data-slop-export="hide">{addError}</p>{/if}
  <footer class="slop-footer">
    <span>Your list, at your pace.</span>
  </footer>
</main>
