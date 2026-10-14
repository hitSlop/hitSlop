<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { Value } from "hitslop";
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import type schema from "./schema";
  import { groceryView } from "./items";
  import Check from "@lucide/svelte/icons/check";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  type GroceryItem = Value<typeof schema.descriptor>["items"][number];

  const { items: visible, done, total, memo } = $derived(groceryView(snapshot));
</script>

<article class="export-door" aria-label="Exported grocery list">
  <section class="notepad">
    <div class="title-row">
      <h1 class="title">{snapshot.title || "Groceries"}</h1>
      <span class="count">{done}/{total} done</span>
    </div>
    {@render rows(visible)}
  </section>
  {#if memo}
    <aside class="sticky">
      <span class="pin" aria-hidden="true"></span>
      <p class="memo">{previewText(snapshot.stickyNote, preview, 140)}</p>
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
