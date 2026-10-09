<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Brand from "./Brand.svelte";
  import Progress from "./Progress.svelte";
  import doc, { checklistView } from "./schema";
  const { visible, finished, ratio } = $derived(checklistView(doc.current));
</script>

<main class="checklist-shell checklist-export">
  <Brand />
  <section class="checklist-paper" aria-label="Your checklist">
    <div class="checklist-heading">
      <p class="checklist-eyebrow">A little less on your mind.</p>
      <h1 class="checklist-title">{doc.current.title || "Untitled list"}</h1>
      <Progress total={visible.length} {finished} fill={ratio} />
    </div>
    <ol class="checklist-list">
      {#each visible as task (task.$id)}
        <li class="checklist-row" data-done={task.done}>
          <span class="checklist-box" aria-label={task.done ? "Complete" : "Incomplete"}>
            {#if task.done}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5" /></svg>{/if}
          </span>
          <span class="checklist-task-text">{task.text || "Untitled task"}</span>
        </li>
      {/each}
    </ol>
    {#if !visible.length}
      <div class="checklist-empty"><Check size={30} /><h2>A little breathing room.</h2></div>
    {/if}
  </section>
</main>
