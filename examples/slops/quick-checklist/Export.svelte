<script lang="ts">
import Brand from "./Brand.svelte";
import { checklistView } from "./model";
import Check from "@lucide/svelte/icons/check";
import doc from "./schema";
// Previews and exports render saved state in a fresh page, so both show the active list.
let {}: { mode: "preview" | "export" } = $props();
const { visible, finished } = $derived(checklistView(doc.current));
</script>

<article class="checklist-shell">
  <Brand />
  <section class="checklist-paper" aria-label="Exported checklist">
    <div class="checklist-heading">
      <p class="checklist-eyebrow">A little less on your mind.</p>
      <h1 class="checklist-title" style:white-space="pre-wrap" style:overflow-wrap="anywhere">{doc.current.title || "Untitled list"}</h1>
      <div class="checklist-progress"><span>{visible.length - finished} left to do</span><span>{finished} / {visible.length} done</span></div>
      <div class="checklist-track"><div style:width={`${visible.length ? finished / visible.length * 100 : 0}%`}></div></div>
    </div>
    <ol class="checklist-list">
      {#each visible as task (task.$id)}
        <li class="checklist-row" data-done={task.done}>
          <span data-checkbox-root data-state={task.done ? "checked" : "unchecked"} aria-label={task.done ? "Complete" : "Incomplete"}>{#if task.done}<Check size={17} strokeWidth={3} />{/if}</span>
          <span class="checklist-task-text" style:white-space="pre-wrap">{task.text || "Untitled task"}</span>
        </li>
      {/each}
    </ol>
    {#if !visible.length}<div class="checklist-empty"><Check size={30} /><h2>A little breathing room.</h2></div>{/if}
  </section>
</article>
