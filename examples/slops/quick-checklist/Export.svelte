<script lang="ts">
import Brand from "./Brand.svelte";
import { checklistView } from "./model";
import { ui } from "./ui.svelte";
import Check from "@lucide/svelte/icons/check";
import doc from "./schema";
let { mode }: { mode: "preview" | "export" } = $props();
const view = $derived(checklistView(doc.current));
// A preview stands for the document, so it shows the active list; an export shows the tab
// the person chose.
const showFiled = $derived(mode === "export" && ui.activeView === "filed");
const exported = $derived(showFiled ? view.filed : view.visible);
const exportFinished = $derived(exported.filter(task => task.done).length);
</script>

<article class="checklist-shell">
  <Brand />
  <section class="checklist-paper" aria-label="Exported checklist">
    <div class="checklist-heading">
      <p class="checklist-eyebrow">{showFiled ? "Filed tasks" : "A little less on your mind."}</p>
      <h1 class="checklist-title" style:white-space="pre-wrap" style:overflow-wrap="anywhere">{doc.current.title || "Untitled list"}</h1>
      <div class="checklist-progress"><span>{showFiled ? `${exported.length} filed` : `${exported.length - exportFinished} left to do`}</span><span>{exportFinished} / {exported.length} done</span></div>
      <div class="checklist-track"><div style:width={`${exported.length ? exportFinished / exported.length * 100 : 0}%`}></div></div>
    </div>
    <ol class="checklist-list">
      {#each exported as task (task.$id)}
        <li class="checklist-row" data-done={task.done}>
          <span data-checkbox-root data-state={task.done ? "checked" : "unchecked"} aria-label={task.done ? "Complete" : "Incomplete"}>{#if task.done}<Check size={17} strokeWidth={3} />{/if}</span>
          <span class="checklist-task-text" style:white-space="pre-wrap">{task.text || "Untitled task"}</span>
        </li>
      {/each}
    </ol>
    {#if !exported.length}<div class="checklist-empty"><Check size={30} /><h2>{showFiled ? "No filed tasks yet." : "A little breathing room."}</h2></div>{/if}
  </section>
</article>
