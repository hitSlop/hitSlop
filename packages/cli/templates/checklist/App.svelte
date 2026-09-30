<script lang="ts">
  import { tick } from "svelte";
  import { Checkbox } from "bits-ui";
  import { useDocument, bindText } from "@hitslop/document/svelte";
  import { checklist } from "./schema";

  const doc = useDocument(checklist);
  const { title, tasks } = doc.fields;
  const taskInputs = new Map<string, HTMLInputElement>();
  function taskInput(node: HTMLInputElement, id: string) {
    taskInputs.set(id, node);
    return { destroy() { taskInputs.delete(id); } };
  }
  let newTask = $state("");
  const complete = $derived(doc.current.tasks.filter(task => task.done).length);
  let adding = $state(false);
  async function add() {
    if (!newTask.trim() || adding) return;
    const submitted = newTask;
    adding = true;
    try {
      const { id } = await tasks.insert({ text: submitted.trim(), done: false });
      if (newTask === submitted) newTask = "";
      await tick();
      taskInputs.get(id)?.focus();
    } catch { /* The runtime reports the rejection; keep input for retry. */ }
    finally { adding = false; }
  }
</script>

<main class="slop-paper">
  <div class="slop-eyebrow">Little checklist</div>
  <input class="slop-title" aria-label="List title" use:bindText={title} />
  <p class="slop-summary">{complete} of {doc.current.tasks.length} done. One thing at a time.</p>
  {#if !doc.current.tasks.length}<p class="slop-summary">A little breathing room. Add your first task below.</p>{/if}
  <ul class="slop-list">
    {#each doc.current.tasks as task, index (task.$id)}
      <li class="slop-row" data-task-id={task.$id}>
        <Checkbox.Root class="slop-check" aria-label={`Complete ${task.text}`} checked={task.done}
          onCheckedChange={(value) => doc.at(task).done.set(value).catch(() => {})}>
          {#if task.done}<span aria-hidden="true">✓</span>{/if}
        </Checkbox.Root>
        <input class="slop-text" data-done={task.done} aria-label="Task text" use:bindText={doc.at(task).text} use:taskInput={task.$id} />
        <div class="slop-actions" data-slop-export="hide">
          <button class="slop-small" aria-label="Move task up" disabled={index === 0} onclick={() => tasks.move(task.$id, { before: doc.current.tasks[index - 1]!.$id }).catch(() => {})}>↑</button>
          <button class="slop-small" aria-label="Move task down" disabled={index === doc.current.tasks.length - 1} onclick={() => tasks.move(task.$id, { after: doc.current.tasks[index + 1]!.$id }).catch(() => {})}>↓</button>
          <button class="slop-small" aria-label="Delete task" onclick={() => tasks.remove(task.$id).catch(() => {})}>×</button>
        </div>
      </li>
    {/each}
  </ul>
  <form class="slop-add" onsubmit={(event) => { event.preventDefault(); return add(); }} data-slop-export="hide">
    <input class="slop-entry" aria-label="New task" placeholder="Something to do…" bind:value={newTask} />
    <button class="slop-button" disabled={adding || !newTask.trim()}>Add</button>
  </form>
  <footer class="slop-footer">
    <span>Your list, at your pace.</span>
  </footer>
</main>
