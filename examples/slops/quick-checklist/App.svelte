<script lang="ts">
import Brand from "./Brand.svelte";
import { checklistView } from "./model";
import { ui } from "./ui.svelte";

  import { useDocument, bindText } from "@hitslop/document/svelte";
  import { onDestroy, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { DropdownMenu, Tabs } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Archive from "@lucide/svelte/icons/archive";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import schema from "./schema";

  const doc = useDocument(schema);
const { visible, filed, finished, ratio } = $derived(checklistView(doc.current, ui.activeView));

  let draft = $state("");
  let composer = $state<HTMLInputElement>();
  let notice = $state("");
  let openMenu = $state<string | null>(null);
  let menuAnchor = $state<HTMLElement | null>(null);
  // Only the row being edited is a live textarea; the rest render as text (WebKit
  // form controls are too expensive to mount by the thousand).
  let editing = $state<{ id: string; caret: number | null } | null>(null);


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
    try {
      await doc.fields.tasks.insert({text, done:false, archived:false});
      if (draft === submitted) draft = "";
      composer?.focus();
    } catch { /* The runtime reports failures; keep the composer for retry. */ }
    finally { adding = false; }
  }
  async function move(id: string, direction: -1 | 1) {
    const index = visible.findIndex(task => task.$id === id);
    const neighbor = visible[index + direction]; if (!neighbor) return;
    try { await doc.fields.tasks.move(id, direction === -1 ? {before:neighbor.$id} : {after:neighbor.$id}); } catch {}
  }
  async function remove(id: string) {
    try { await doc.fields.tasks.remove(id); notice = "Task removed."; composer?.focus(); } catch {}
  }
  async function fileFinished() {
    const done = visible.filter(task => task.done);
    try {
      await doc.change(tx => { for (const task of done) tx.at(task).archived.set(true); });
      notice = `${done.length} ${done.length === 1 ? "task" : "tasks"} filed.`;
    } catch {}
  }
  async function restore(task: (typeof filed)[number]) {
    try {
      await doc.change(tx => { const row = tx.at(task); row.archived.set(false); row.done.set(false); });
      notice = "Task moved back to your list.";
    } catch {}
  }
  async function toggle(id: string, input: HTMLInputElement) {
    const checked = input.checked;
    try { await doc.fields.tasks.item(id).done.set(checked); }
    catch { input.checked = !checked; }
  }
  function openActions(id: string, button: HTMLElement) {
    if (openMenu === id) { openMenu = null; return; }
    menuAnchor = button;
    openMenu = id;
  }
  function startEditing(id: string, event?: MouseEvent) {
    let caret: number | null = null;
    const target = event?.currentTarget as HTMLElement | undefined;
    const range = event && (document as any).caretRangeFromPoint?.(event.clientX, event.clientY);
    if (target && range && target.contains(range.startContainer) && range.startContainer.nodeType === Node.TEXT_NODE) caret = range.startOffset;
    editing = { id, caret };
  }
  function focusEditor(node: HTMLTextAreaElement, caret: number | null) {
    node.focus();
    const at = Math.min(caret ?? node.value.length, node.value.length);
    node.setSelectionRange(at, at);
  }
  // CSS auto-grow: the wrapper's ::after mirrors the text, so no per-row measuring. Only
  // the row being edited carries the mirror; a text row sizes itself.
  function mirror(event: Event) {
    const field = event.currentTarget as HTMLTextAreaElement;
    field.parentElement!.dataset.value = field.value;
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
      <div class="checklist-grow title" data-value={doc.current.title}>
        <textarea
          class="checklist-title"
          aria-label="Checklist title"
          rows="1"
          oninput={mirror}
          use:bindText={doc.fields.title}
          placeholder="Name your list"
        ></textarea>
      </div>
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
            <li class="checklist-row" data-done={task.done}>
              <label class="checklist-box">
                <input
                  type="checkbox"
                  checked={task.done}
                  onchange={(event) => toggle(task.$id, event.currentTarget)}
                  aria-label={`Mark ${task.text || "untitled task"} ${task.done ? "incomplete" : "complete"}`}
                />
                {#if task.done}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5" /></svg>{/if}
              </label>
              <div class="checklist-grow task" data-value={editing?.id === task.$id ? task.text : undefined}>
                {#if editing?.id === task.$id}
                  <textarea
                    class="checklist-task-text"
                    aria-label={`Task ${index + 1}`}
                    rows="1"
                    oninput={mirror}
                    use:bindText={doc.at(task).text}
                    use:focusEditor={editing.caret}
                    onblur={() => { if (editing?.id === task.$id) editing = null; }}
                    placeholder="Untitled task"
                    onkeydown={(event) => {
                      if (event.key === "Enter" && !event.shiftKey) {
                        event.preventDefault();
                        composer?.focus();
                      }
                    }}
                  ></textarea>
                {:else}
                  <div
                    class="checklist-task-text"
                    role="textbox"
                    tabindex="0"
                    aria-label={`Task ${index + 1}`}
                    onfocus={() => startEditing(task.$id)}
                    onmousedown={(event) => { event.preventDefault(); startEditing(task.$id, event); }}
                  >{#if task.text}{task.text}{:else}<span class="checklist-placeholder">Untitled task</span>{/if}</div>
                {/if}
              </div>
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
