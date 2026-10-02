<script lang="ts">
  import doc from "./schema";
  import Check from "@lucide/svelte/icons/check";
  import { type GroceryItem } from "./schema";
  import { ui } from "./ui.svelte";

  const visible = $derived(ui.filter === "All" ? doc.current.items : doc.current.items.filter((item) => item.category === ui.filter));
</script>

<article class="export-door" aria-label="Exported grocery list">
  <section class="notepad">
    <div class="title-row">
      <h1 class="title">{doc.current.title || "Groceries"}</h1>
      <span class="count">{visible.filter((item) => item.done).length}/{visible.length} done</span>
    </div>
    {@render rows(visible)}
  </section>
  {#if doc.current.stickyNote.trim()}
    <aside class="sticky">
      <span class="pin" aria-hidden="true"></span>
      <p class="memo">{doc.current.stickyNote}</p>
    </aside>
  {/if}
</article>

{#snippet rows(items: readonly GroceryItem[])}
  <ul class="list">
    {#each items as item (item.$id)}
      <li class="row" data-done={item.done}>

          <span data-checkbox-root data-state={item.done ? "checked" : "unchecked"}>{#if item.done}<Check size={10} strokeWidth={3} />{/if}</span>
          <span class="item-text">{item.text || "Untitled item"}</span>

        <span class="badge">{item.category}</span>

      </li>
    {:else}
      <li class="empty">
        <h2>{ui.filter === "All" ? "The pad is empty." : `Nothing in ${ui.filter} yet.`}</h2>

      </li>
    {/each}
  </ul>
{/snippet}
