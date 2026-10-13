<script lang="ts">
  import type { WidgetProps } from "hitslop/svelte";
  import doc from "./schema";

  let { family }: WidgetProps = $props();
  const remaining = $derived(doc.current.items.filter(item => !item.done));
  const memo = $derived(doc.current.stickyNote.trim());
  const rows = $derived(remaining.slice(0, family === "large" ? (memo ? 6 : 8) : family === "medium" ? 3 : 2));
  function shorten(value: string, limit: number) {
    const characters = Array.from(value);
    return characters.length > limit ? `${characters.slice(0, limit - 1).join("")}…` : value;
  }
</script>

<main class="grocery-widget" data-family={family}>
  <section class="grocery-widget-pad">
    <header class="grocery-widget-header">
      <h1>{shorten(doc.current.title.trim() || "Groceries", family === "small" ? 12 : 25)}</h1>
      <p>{remaining.length} to get</p>
    </header>
    <ul class="grocery-widget-items">
      {#each rows as item (item.$id)}
        <li>
          <span class="grocery-widget-box" aria-hidden="true"></span>
          <span class="grocery-widget-name">{shorten(item.text.trim() || "Untitled item", family === "small" ? 15 : 25)}</span>
          {#if family !== "small"}<span class="grocery-widget-category">{item.category}</span>{/if}
        </li>
      {:else}
        <li class="grocery-widget-empty">{doc.current.items.length ? "All picked up." : "Your next trip starts here."}</li>
      {/each}
    </ul>
    {#if remaining.length > rows.length}<p class="grocery-widget-more">+ {remaining.length - rows.length} more on the list</p>{/if}
  </section>
  {#if family === "large" && memo}
    <aside class="grocery-widget-memo">
      <span class="grocery-widget-pin" aria-hidden="true"></span>
      <p>{shorten(memo, 85)}</p>
    </aside>
  {/if}
</main>

<style>
  .grocery-widget { box-sizing: border-box; display: flex; flex-direction: column; gap: 12px; width: 100%; height: 100%; padding: 12px; overflow: hidden; color: var(--slop-ink); background: var(--slop-surface); font-family: sans-serif; }
  .grocery-widget-pad { display: flex; flex-direction: column; flex: 1; min-height: 0; padding: 12px 14px; border-top: 5px solid var(--slop-accent); border-radius: 5px; background: var(--slop-paper); box-shadow: 0 3px 8px #23222018; }
  .grocery-widget-header { display: flex; align-items: baseline; gap: 10px; justify-content: space-between; padding-bottom: 7px; border-bottom: 2px solid var(--slop-ink); }
  .grocery-widget-header h1 { flex: 1; min-width: 0; margin: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; font-family: serif; font-size: 24px; font-weight: 700; line-height: 1.1; }
  .grocery-widget-header p { flex-shrink: 0; margin: 0; font-size: 11px; font-weight: 700; color: var(--slop-muted); }
  .grocery-widget-items { list-style: none; margin: 3px 0 0; padding: 0; }
  .grocery-widget-items li { display: flex; align-items: center; gap: 7px; min-height: 27px; border-bottom: 1px solid var(--slop-rule); font-size: 13px; line-height: 1.2; }
  .grocery-widget-box { flex-shrink: 0; width: 11px; height: 11px; border: 1px solid var(--slop-muted); border-radius: 3px; }
  .grocery-widget-name { flex: 1; min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .grocery-widget-category { flex-shrink: 0; color: var(--slop-muted); font-size: 9px; }
  .grocery-widget-items .grocery-widget-empty { border: 0; min-height: 44px; color: var(--slop-muted); }
  .grocery-widget-more { margin: 7px 0 0; color: var(--slop-muted); font-size: 10px; }
  .grocery-widget-memo { position: relative; flex-shrink: 0; padding: 11px 14px; border: 1px solid var(--slop-stickyBorder); border-radius: 5px; background: var(--slop-sticky); color: var(--slop-stickyInk); }
  .grocery-widget-pin { position: absolute; top: -4px; right: 19px; width: 8px; height: 8px; border-radius: 50%; background: var(--slop-pin); }
  .grocery-widget-memo p { margin: 0; font-size: 12px; line-height: 1.3; }
  [data-family="small"].grocery-widget { padding: 8px; }
  [data-family="small"] .grocery-widget-pad { padding: 9px 10px; }
  [data-family="small"] .grocery-widget-header { align-items: flex-start; flex-direction: column; gap: 3px; padding-bottom: 5px; }
  [data-family="small"] .grocery-widget-header h1 { flex: none; width: 100%; font-size: 20px; }
  [data-family="small"] .grocery-widget-items li { font-size: 11px; min-height: 24px; }
  [data-family="small"] .grocery-widget-more { margin-top: 5px; font-size: 9px; }
  [data-family="medium"].grocery-widget { padding: 8px; }
  [data-family="medium"] .grocery-widget-pad { padding: 8px 13px; }
  [data-family="medium"] .grocery-widget-header { padding-bottom: 5px; }
  [data-family="medium"] .grocery-widget-header h1 { font-size: 21px; }
  [data-family="medium"] .grocery-widget-items li { min-height: 24px; }
  [data-family="medium"] .grocery-widget-more { margin-top: 5px; }
</style>
