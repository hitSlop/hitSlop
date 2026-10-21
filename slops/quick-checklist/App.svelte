<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { flip } from "svelte/animate";
  import { fade } from "svelte/transition";
  import { prefersReducedMotion } from "svelte/motion";
  import Brand from "./Brand.svelte";
  import Progress from "./Progress.svelte";
  import schema from "./schema";
  import { addTask, checklistView, fileFinished as collectFinished, restoreTask } from "./tasks";
  import { EditableText, draft, getDocument, getEnvironment, motion, notify, haptics, popoverTransition } from "hitslop/svelte";
  import { DropdownMenu, Tabs } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Archive from "@lucide/svelte/icons/archive";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";

  const doc = getDocument(schema);
  const environment = getEnvironment();
  let activeView = $state<"tasks" | "filed">("tasks");
  const { visible, filed, finished, ratio } = $derived(checklistView(doc.current));
  const fill = motion(() => ratio);
  const newTask = draft(text => doc.change(tx => addTask(tx, text)));

  let composer = $state<HTMLInputElement>();
  let openMenu = $state<string | null>(null);
  let menuTask = $state<string | null>(null);
  let menuAnchor = $state<HTMLElement | null>(null);
  let tasksTab = $state<HTMLButtonElement | null>(null);
  const menuIndex = $derived(menuTask ? visible.findIndex(task => task.$id === menuTask) : -1);
  $effect(() => { if (openMenu && menuIndex < 0) openMenu = null; });

  async function add(event: SubmitEvent) {
    if (await newTask.submit(event)) composer?.focus();
  }
  async function move(id: string, direction: -1 | 1) {
    const index = visible.findIndex(task => task.$id === id);
    const task = visible[index], neighbor = visible[index + direction];
    if (!task || !neighbor) return;
    await doc.fields.tasks.move(task, direction === -1 ? { before: neighbor } : { after: neighbor });
  }
  async function remove(id: string) {
    await doc.fields.tasks.remove(id);
    notify("Task removed.");
    if (environment.current.device !== "phone") composer?.focus();
    else tasksTab?.focus();
  }
  async function fileFinished() {
    const count = await doc.change(tx => collectFinished(tx, doc.current));
    if (count) haptics.notification("success");
    notify(`${count} ${count === 1 ? "task" : "tasks"} filed.`);
  }
  async function restore(task: (typeof filed)[number]) {
    await doc.change(tx => restoreTask(tx, task));
    notify("Task moved back to your list.");
  }
  function openActions(id: string, button: HTMLElement) {
    if (openMenu === id) { openMenu = null; return; }
    menuAnchor = button;
    menuTask = id;
    openMenu = id;
  }
</script>

<main class="checklist-shell">
  <Brand />
  <section class="checklist-paper" aria-label="Your checklist">
    <div class="checklist-heading">
      <p class="checklist-eyebrow">A little less on your mind.</p>
      <EditableText
        class="checklist-title"
        field={doc.fields.title}
        label="Checklist title"
        placeholder="Name your list"
      />
      <Progress total={visible.length} {finished} fill={fill.current} live />
    </div>
    <form class="checklist-composer" onsubmit={add}>
      <input
        bind:this={composer}
        bind:value={newTask.value}
        aria-label="New task"
        placeholder="Add a little thing…"
        enterkeyhint="enter"
        autocapitalize="sentences"
        autocomplete="off"
      />
      <button type="submit" aria-label="Add task" disabled={!newTask.ready}
        onpointerdown={(event) => { if (document.activeElement === composer) event.preventDefault(); }}><Plus size={20} /></button>
    </form>
    <Tabs.Root
      value={activeView}
      onValueChange={(value) => {
        if ((value === "tasks" || value === "filed") && value !== activeView) { activeView = value; haptics.selection(); }
      }}
    >
      <Tabs.List
        class="checklist-tabs"
        aria-label="Checklist views"
      >
        <Tabs.Trigger value="tasks" class="checklist-tab" bind:ref={tasksTab}
          >To do <span>{visible.length}</span></Tabs.Trigger
        >
        <Tabs.Trigger value="filed" class="checklist-tab"
          ><Archive size={14} /> Filed <span>{filed.length}</span></Tabs.Trigger
        >
      </Tabs.List>
    </Tabs.Root>
    <div class={["checklist-scroller", activeView === "filed" && "checklist-filed-view"]}>
      {#if activeView === "tasks"}
        <ol class="checklist-list">
          {#each visible as task, index (task.$id)}
            {@const row = doc.at(task)}
            <li class="checklist-row" data-done={task.done}
              animate:flip={{ duration: prefersReducedMotion.current ? 0 : 180 }}
              transition:fade={{ duration: prefersReducedMotion.current ? 0 : 120 }}>
              <label class="checklist-box">
                <input
                  type="checkbox"
                  bind:checked={row.done.value}
                  onchange={() => haptics.selection()}
                  aria-label={`Mark ${task.text || "untitled task"} ${task.done ? "incomplete" : "complete"}`}
                />
                {#if task.done}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5" /></svg>{/if}
              </label>
              <EditableText
                class="checklist-task-text"
                field={row.text}
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
        <DropdownMenu.Root open={openMenu !== null} onOpenChange={(open) => { if (!open) openMenu = null; }}
          onOpenChangeComplete={(open) => { if (!open && !openMenu) menuTask = null; }}>
          <DropdownMenu.Portal>
            <DropdownMenu.Content
              forceMount
              customAnchor={menuAnchor}
              sideOffset={5}
              align="end"
              onInteractOutside={(event) => { if (menuAnchor?.contains(event.target as Node)) event.preventDefault(); }}
              onCloseAutoFocus={(event) => { event.preventDefault(); if (menuAnchor?.isConnected) menuAnchor.focus(); else tasksTab?.focus(); }}
            >
              {#snippet child({ wrapperProps, props, open })}
              {#if open}
              <div {...wrapperProps}><div {...props} class="checklist-menu" transition:popoverTransition>
              {#if menuTask}
                {@const id = menuTask}
                <DropdownMenu.Item disabled={menuIndex <= 0} onSelect={() => move(id, -1)}>Move up</DropdownMenu.Item>
                <DropdownMenu.Item disabled={menuIndex === visible.length - 1} onSelect={() => move(id, 1)}>Move down</DropdownMenu.Item>
                <DropdownMenu.Separator />
                <DropdownMenu.Item onSelect={() => remove(id)}>Remove task</DropdownMenu.Item>
              {/if}
              </div></div>
              {/if}
              {/snippet}
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
        >{activeView === "tasks"
          ? "Enter to add. Check to finish."
          : "Restore anything you need again."}</span
      >{#if activeView === "tasks"}<button
          onclick={fileFinished}
          disabled={!finished}
          ><Archive size={15} /> File finished{finished
            ? ` (${finished})`
            : ""}</button
        >{/if}
    </div>
  </section>
</main>
