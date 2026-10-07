<script lang="ts">
import Brand from "./Brand.svelte";
import { checklistView } from "./model";
import { ui } from "./ui.svelte";

  import { EditableText } from "hitslop/svelte";
  import { onDestroy, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { DropdownMenu, Tabs } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Archive from "@lucide/svelte/icons/archive";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import doc from "./schema";
  import * as actions from "./commands";

const { visible, filed, finished, ratio } = $derived(checklistView(doc.current));

  let draft = $state("");
  let composer = $state<HTMLInputElement>();
  let notice = $state("");
  let openMenu = $state<string | null>(null);
  let menuAnchor = $state<HTMLElement | null>(null);


  const menuIndex = $derived(openMenu ? visible.findIndex(task => task.$id === openMenu) : -1);
  $effect(() => { if (openMenu && menuIndex < 0) openMenu = null; });


  const fill = new Tween(untrack(() => ratio), { duration: 280, easing: cubicOut });
  let initialized = false;
  $effect(() => {
    void fill.set(ratio, { duration: !initialized || prefersReducedMotion.current ? 0 : 280, delay: 0 });
    initialized = true;
  });
  onDestroy(() => { void fill.set(fill.target, {duration:0,delay:0}); });
  $effect(() => { if (notice) { const timer = setTimeout(() => notice = "", 4000); return () => clearTimeout(timer); } });


  let adding = $state(false);
  async function addTask() {
    const submitted = draft;
    const text = submitted.trim(); if (!text || adding) return;
    adding = true;
    // A refused insert is reported by the runtime and leaves the draft for retry.
    try {
      await actions.addTask({ text });
      if (draft === submitted) draft = "";
      composer?.focus();
    } finally { adding = false; }
  }
  async function move(id: string, direction: -1 | 1) {
    const index = visible.findIndex(task => task.$id === id);
    const neighbor = visible[index + direction]; if (!neighbor) return;
    await doc.fields.tasks.move(id, direction === -1 ? {before:neighbor.$id} : {after:neighbor.$id});
  }
  async function remove(id: string) {
    await doc.fields.tasks.remove(id);
    notice = "Task removed.";
    composer?.focus();
  }
  async function fileFinished() {
    const count = await actions.archiveFinished();
    notice = `${count} ${count === 1 ? "task" : "tasks"} filed.`;
  }
  async function restore(task: (typeof filed)[number]) {
    await actions.restoreTask({ id: task.$id });
    notice = "Task moved back to your list.";
  }
  function openActions(id: string, button: HTMLElement) {
    if (openMenu === id) { openMenu = null; return; }
    menuAnchor = button;
    openMenu = id;
  }

</script>


<main
  class="checklist-shell"
>
  <Brand />
  <section
    class="checklist-paper"
    aria-label="Your checklist"
  >
    <div class="checklist-heading">
      <p class="checklist-eyebrow">A little less on your mind.</p>
      <EditableText
        class="checklist-title"
        handle={doc.fields.title}
        text={doc.current.title}
        label="Checklist title"
        placeholder="Name your list"
      />
      <div class="checklist-progress">
        <span aria-live="polite"
          >{visible.length && finished === visible.length
            ? "All done. Nicely done."
            : `${visible.length - finished} left to do`}</span
        >
        <span>{finished} / {visible.length} done</span>
      </div>
      <div class="checklist-track" aria-hidden="true">
        <div style:transform={`scaleX(${fill.current / 100})`}></div>
      </div>
    </div>
    <form
      class="checklist-composer"
      onsubmit={(event) => {
        event.preventDefault();
        addTask();
      }}
    >
      <input
        bind:this={composer}
        bind:value={draft}
        aria-label="New task"
        placeholder="Add a little thing…"
      />
      <button
        type="submit"
        aria-label="Add task"
        disabled={adding || !draft.trim()}
        ><Plus size={20} /></button
      >
    </form>
    <Tabs.Root
      value={ui.activeView}
      onValueChange={(value) => {
        if (value === "tasks" || value === "filed") ui.activeView = value;
      }}
    >
      <Tabs.List
        class="checklist-tabs"
        aria-label="Checklist views"
      >
        <Tabs.Trigger value="tasks" class="checklist-tab"
          >To do <span>{visible.length}</span></Tabs.Trigger
        >
        <Tabs.Trigger value="filed" class="checklist-tab"
          ><Archive size={14} /> Filed <span>{filed.length}</span></Tabs.Trigger
        >
      </Tabs.List>
    </Tabs.Root>
    <div class={`${"checklist-scroller"} ${ui.activeView === "filed" ? "checklist-filed-view" : ""}`}>
      {#if ui.activeView === "tasks"}
        <ol class="checklist-list">
          {#each visible as task, index (task.$id)}
            {@const row = doc.at(task)}
            <li class="checklist-row" data-done={task.done}>
              <label class="checklist-box">
                <input
                  type="checkbox"
                  bind:checked={row.done.value}
                  aria-label={`Mark ${task.text || "untitled task"} ${task.done ? "incomplete" : "complete"}`}
                />
                {#if task.done}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5" /></svg>{/if}
              </label>
              <EditableText
                class="checklist-task-text"
                handle={row.text}
                text={task.text}
                label={`Task ${index + 1}`}
                placeholder="Untitled task"
                onenter={() => composer?.focus()}
              />
              <button
                class="checklist-more"
                aria-label={`Actions for ${task.text || "untitled task"}`}
                aria-haspopup="menu"
                aria-expanded={openMenu === task.$id}
                onclick={(event) => openActions(task.$id, event.currentTarget)}
              ><Ellipsis size={19} /></button>
            </li>
          {/each}
        </ol>
        <DropdownMenu.Root open={openMenu !== null} onOpenChange={(open) => { if (!open) openMenu = null; }}>
          <DropdownMenu.Portal>
            <DropdownMenu.Content
              class="checklist-menu"
              customAnchor={menuAnchor}
              sideOffset={5}
              align="end"
              onInteractOutside={(event) => { if (menuAnchor?.contains(event.target as Node)) event.preventDefault(); }}
              onCloseAutoFocus={(event) => { event.preventDefault(); menuAnchor?.focus(); }}
            >
              {#if openMenu}
                {@const id = openMenu}
                <DropdownMenu.Item disabled={menuIndex <= 0} onSelect={() => move(id, -1)}>Move up</DropdownMenu.Item>
                <DropdownMenu.Item disabled={menuIndex === visible.length - 1} onSelect={() => move(id, 1)}>Move down</DropdownMenu.Item>
                <DropdownMenu.Separator />
                <DropdownMenu.Item onSelect={() => remove(id)}>Remove task</DropdownMenu.Item>
              {/if}
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
        {#if !visible.length}<div class="checklist-empty">
            <Check size={30} />
            <h2>A little breathing room.</h2>
            <p>Catch your next small task above.</p>
          </div>{/if}
      {:else}
        <div class="checklist-filed-list">
          {#each filed as task (task.$id)}<div class="checklist-filed-row">
              <span>{task.text || "Untitled task"}</span><button
                onclick={() => restore(task)}
                aria-label={`Restore ${task.text || "untitled task"}`}
                ><RotateCcw size={16} /> Restore</button
              >
            </div>
          {:else}<div class="checklist-empty">
              <Archive size={30} />
              <h2>No filed tasks yet.</h2>
              <p>Finish a task, then file it from your to-do list.</p>
            </div>{/each}
        </div>
      {/if}
    </div>
    <div class="checklist-paper-foot">
      <span
        >{ui.activeView === "tasks"
          ? "Enter to add. Check to finish."
          : "Restore anything you need again."}</span
      >{#if ui.activeView === "tasks"}<button
          onclick={fileFinished}
          disabled={!finished}
          ><Archive size={15} /> File finished{finished
            ? ` (${finished})`
            : ""}</button
        >{/if}
    </div>
  </section>
  {#if notice}<div class="checklist-notice" role="status">{notice}</div>{/if}
</main>
