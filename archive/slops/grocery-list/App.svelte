<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Select, Button, Tooltip } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import schema, { categories, type GroceryItem } from "./schema";

  type Category = (typeof categories)[number];
  const categoryItems = categories.map((value) => ({ value, label: value }));

  const doc = useDocument(schema);
  let filter = $state<"All" | Category>("All");
  let draft = $state("");
  let aisle = $state<Category>("Produce");
  let composer = $state<HTMLInputElement>();

  const visible = $derived(filter === "All" ? doc.current.items : doc.current.items.filter((item) => item.category === filter));
  const doneCount = $derived(doc.current.items.filter((item) => item.done).length);
  const totalCount = $derived(doc.current.items.length);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function isCategory(value: string): value is Category {
    return categories.some((category) => category === value);
  }

  function addItem() {
    const text = draft.trim();
    if (!text) return;
    doc.fields.items.insert({ text, category: aisle, done: false });
    draft = "";
    composer?.focus();
  }

  function removeItem(id: string) {
    doc.fields.items.remove(id);
  }

  function clearDone() {
    const done = doc.current.items.filter((item) => item.done);
    if (!done.length) return;
    doc.change((tx) => {
      for (const item of done) tx.fields.items.remove(item.$id);
    }, { message: "Clear checked items" });
  }
</script>

{#snippet rows(items: readonly GroceryItem[], editable: boolean)}
  <ul class="list">
    {#each items as item (item.$id)}
      <li class="row" data-done={item.done} animate:flip={{ duration: editable ? flipMs : 0 }}>
        {#if editable}
          <Checkbox.Root
            checked={item.done}
            onCheckedChange={(checked) => doc.at(item).done.set(checked === true)}
            aria-label={`Mark ${item.text || "untitled item"} ${item.done ? "still needed" : "in the cart"}`}
          >
            {#snippet children({ checked })}{#if checked}<Check size={12} strokeWidth={3} />{/if}{/snippet}
          </Checkbox.Root>
          <input class="item-text" aria-label="Grocery item text" use:bindText={doc.at(item).text} />
        {:else}
          <span data-checkbox-root data-state={item.done ? "checked" : "unchecked"}>{#if item.done}<Check size={10} strokeWidth={3} />{/if}</span>
          <span class="item-text">{item.text || "Untitled item"}</span>
        {/if}
        <span class="badge">{item.category}</span>
        {#if editable}
          <Tooltip.Root>
            <Tooltip.Trigger class="remove" data-slop-export="hide" aria-label={`Delete ${item.text || "untitled item"}`} onclick={() => removeItem(item.$id)}>
              <Trash2 size={13} />
            </Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content class="tooltip" sideOffset={6}>Remove from the pad</Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>
        {/if}
      </li>
    {:else}
      <li class="empty">
        <h2>{filter === "All" ? "The pad is empty." : `Nothing in ${filter} yet.`}</h2>
        {#if editable}<p>{filter === "All" ? "Add milk, bread, or whatever you need." : "Add an item to this aisle, or switch back to All."}</p>{/if}
      </li>
    {/each}
  </ul>
{/snippet}

<Slop>
  <Tooltip.Provider>
    <main class="door" data-slop-selection="none" aria-label="Grocery list">
      <div class="magnet banana" aria-hidden="true"><div class="banana-body"><span></span><span></span></div></div>
      <div class="magnet smiley" aria-hidden="true">😊</div>
      <div class="magnet star" aria-hidden="true">★</div>

      <article class="notepad">
        <div class="title-row">
          <input class="title" aria-label="List title" use:bindText={doc.fields.title} placeholder="Groceries" />
          <span class="count">{doneCount}/{totalCount} done</span>
        </div>

        <Tabs.Root value={filter} onValueChange={(value) => { if (value === "All" || isCategory(value)) filter = value; }}>
          <Tabs.List class="tabs" aria-label="Aisle filters" data-slop-export="hide">
            <Tabs.Trigger value="All" class="tab">All</Tabs.Trigger>
            {#each categories as category}
              <Tabs.Trigger value={category} class="tab">{category}</Tabs.Trigger>
            {/each}
          </Tabs.List>
        </Tabs.Root>

        <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addItem(); }}>
          <input bind:this={composer} bind:value={draft} aria-label="New grocery item" placeholder="Add milk, bread…" />
          <Select.Root type="single" value={aisle} items={categoryItems} onValueChange={(value) => { if (isCategory(value)) aisle = value; }}>
            <Select.Trigger class="select-trigger" aria-label="Aisle for new item">
              <Select.Value placeholder="Aisle" />
              <ChevronDown size={12} strokeWidth={2.2} />
            </Select.Trigger>
            <Select.Portal>
              <Select.Content class="select-content" sideOffset={6}>
                <Select.Viewport>
                  {#each categoryItems as item (item.value)}
                    <Select.Item value={item.value} label={item.label}>
                      {#snippet children({ selected })}
                        {item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}
                      {/snippet}
                    </Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
          <Button.Root class="add" type="submit" aria-label="Add item" disabled={!draft.trim()}><Plus size={16} /></Button.Root>
        </form>

        {@render rows(visible, true)}

        <div class="foot" data-slop-export="hide">
          <span>Enter to add. Check when it’s in the cart.</span>
          <Button.Root onclick={clearDone} disabled={!doneCount}>Clear checked{doneCount ? ` (${doneCount})` : ""}</Button.Root>
        </div>
      </article>

      <aside class="sticky">
        <span class="pin" aria-hidden="true"></span>
        <textarea class="memo" aria-label="Sticky reminder" use:bindText={doc.fields.stickyNote} placeholder="A little reminder…"></textarea>
      </aside>
    </main>
  </Tooltip.Provider>

  {#snippet exportView()}
    <article class="export-door" aria-label="Exported grocery list">
      <section class="notepad">
        <div class="title-row">
          <h1 class="title">{doc.current.title || "Groceries"}</h1>
          <span class="count">{visible.filter((item) => item.done).length}/{visible.length} done</span>
        </div>
        {@render rows(visible, false)}
      </section>
      {#if doc.current.stickyNote.trim()}
        <aside class="sticky">
          <span class="pin" aria-hidden="true"></span>
          <p class="memo">{doc.current.stickyNote}</p>
        </aside>
      {/if}
    </article>
  {/snippet}

  {#snippet icon()}
    {@const marks = totalCount > 0 ? Math.round(4 * doneCount / totalCount) : 0}
    <div class="icon-surface" aria-hidden="true">
      <div class="icon-plate">
        <div class="icon-pad">
          <div class="icon-title"></div>
          <div class="icon-lines">
            {#each [0, 1, 2, 3] as index}
              <div class="icon-row">
                <span class="icon-box" data-done={index < marks}></span>
                <i class="icon-line"></i>
              </div>
            {/each}
          </div>
        </div>
        <div class="icon-banana"></div>
        <span class="icon-star">★</span>
      </div>
    </div>
  {/snippet}
</Slop>
