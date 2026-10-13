<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  import Check from "@lucide/svelte/icons/check";
  import Brand from "./Brand.svelte";
  import Progress from "./Progress.svelte";
  import doc from "./schema";
  import { checklistView } from "./tasks";
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
      {#each (preview ? (visible).slice(0, 7) : visible) as task (task.$id)}
        <li class="checklist-row" data-done={task.done}>
          <span class="checklist-box" aria-label={task.done ? "Complete" : "Incomplete"}>
            {#if task.done}<Check size={20} strokeWidth={3.5} />{/if}
          </span>
          <span class="checklist-task-text">{previewText(task.text || "Untitled task", preview, 110)}</span>
        </li>
      {/each}
    </ol>
    {#if !visible.length}
      <div class="checklist-empty"><Check size={30} /><h2>A little breathing room.</h2></div>
    {/if}
  </section>
</main>
