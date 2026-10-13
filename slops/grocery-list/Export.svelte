<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { groceryView } from "./items";
  import Check from "@lucide/svelte/icons/check";

  type GroceryItem = (typeof doc.current.items)[number];

  const { items: visible, done, total, memo } = $derived(groceryView(doc.current));
</script>

<article class="export-door" aria-label="Exported grocery list">
  <section class="notepad">
    <div class="title-row">
      <h1 class="title">{doc.current.title || "Groceries"}</h1>
      <span class="count">{done}/{total} done</span>
    </div>
    {@render rows(visible)}
  </section>
  {#if memo}
    <aside class="sticky">
      <span class="pin" aria-hidden="true"></span>
      <p class="memo">{previewText(doc.current.stickyNote, preview, 140)}</p>
    </aside>
  {/if}
</article>

{#snippet rows(items: readonly GroceryItem[])}
  <ul class="list">
    {#each (preview ? (items).slice(0, 9) : items) as item (item.$id)}
      <li class="row" data-done={item.done}>

          <span data-checkbox-root data-state={item.done ? "checked" : "unchecked"}>{#if item.done}<Check size={10} strokeWidth={3} />{/if}</span>
          <span class="item-text">{item.text || "Untitled item"}</span>

        <span class="badge">{item.category}</span>

      </li>
    {:else}
      <li class="empty">
        <h2>The pad is empty.</h2>

      </li>
    {/each}
  </ul>
{/snippet}
