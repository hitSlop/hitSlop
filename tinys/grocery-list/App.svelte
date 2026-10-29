<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { bindText, draft, getDocument, haptics } from "@hitslop/tinys/svelte";
  import type { Value } from "@hitslop/tinys";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { Tabs, Checkbox, Select, Button, Tooltip } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import schema, { categories } from "./schema";
  import { addItem, clearDone as clearCheckedItems, groceryView, removeItem, setDone, type Category, type Filter } from "./items";

  const doc = getDocument(schema);
  type GroceryItem = Value<typeof schema.descriptor>["items"][number];

  const categoryItems = categories.map((value) => ({ value, label: value }));
  let filter = $state<Filter>("All");
  let aisle = $state<Category>("Produce");
  const newItem = draft((text) => doc.change((tx) => addItem(tx, text, aisle)));
  let composer = $state<HTMLInputElement>();

  const { items: visible, done: doneCount, total: totalCount } = $derived(groceryView(doc.current, filter));
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function isCategory(value: string): value is Category {
    return categories.some((category) => category === value);
  }

  async function addNewItem(event: SubmitEvent) {
    if (await newItem.submit(event)) {
      haptics.impact();
      composer?.focus();
    }
  }

  async function clearDone() {
    if (!doneCount) return;
    await doc.change((tx) => clearCheckedItems(tx, doc.current));
    haptics.notification("success");
  }
</script>

{#snippet rows(items: readonly GroceryItem[], editable: boolean)}
  <ul class="list">
    {#each items as item (item.$id)}
      <li class="row" data-done={item.done} animate:flip={{ duration: editable ? flipMs : 0 }}>
        {#if editable}
          <Checkbox.Root
            checked={item.done}
            onCheckedChange={async (checked) => { await doc.change((tx) => setDone(tx, item.$id, checked === true)); haptics.selection(); }}
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
            <Tooltip.Trigger class="remove" data-tiny-export="hide" aria-label={`Delete ${item.text || "untitled item"}`} onclick={async () => { await doc.change((tx) => removeItem(tx, item.$id)); haptics.impact("medium"); }}>
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

  <Tooltip.Provider>
    <main class="door" aria-label="Grocery list">
      <div class="magnet banana" aria-hidden="true"><div class="banana-body"><span></span><span></span></div></div>
      <div class="magnet smiley" aria-hidden="true">😊</div>
      <div class="magnet star" aria-hidden="true">★</div>

      <article class="notepad">
        <div class="title-row">
          <input class="title" aria-label="List title" use:bindText={doc.fields.title} placeholder="Groceries" />
          <span class="count">{doneCount}/{totalCount} done</span>
        </div>

        <Tabs.Root value={filter} onValueChange={(value) => { if ((value === "All" || isCategory(value)) && value !== filter) { filter = value; haptics.selection(); } }}>
          <Tabs.List class="tabs" aria-label="Aisle filters" data-tiny-export="hide">
            <Tabs.Trigger value="All" class="tab">All</Tabs.Trigger>
            {#each categories as category}
              <Tabs.Trigger value={category} class="tab">{category}</Tabs.Trigger>
            {/each}
          </Tabs.List>
        </Tabs.Root>

        <form class="composer" data-tiny-export="hide" onsubmit={addNewItem}>
          <input bind:this={composer} bind:value={newItem.value} aria-label="New grocery item" placeholder="Add milk, bread…" />
          <Select.Root type="single" value={aisle} items={categoryItems} onValueChange={(value) => { if (isCategory(value) && value !== aisle) { aisle = value; haptics.selection(); } }}>
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
          <Button.Root class="add" type="submit" aria-label="Add item" disabled={!newItem.ready}><Plus size={16} /></Button.Root>
        </form>

        {@render rows(visible, true)}

        <div class="foot" data-tiny-export="hide">
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
