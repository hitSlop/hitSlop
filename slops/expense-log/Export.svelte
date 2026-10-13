<script lang="ts">
  import type { Value } from "hitslop";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { categoryCode, formatDate } from "./shared";
  import { expenseView } from "./items";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  type ExpenseItem = Value<typeof schema.descriptor>["items"][number];
  const today = formatDate(new Date());
  const view = $derived(expenseView(snapshot));
</script>

<article class="exportShell" aria-label="Exported expense receipt">
  {@render tear("top")}
  <div class="paper">
    <header class="masthead">
      <p class="stamp">*** RECEIPT ***</p>
      <h1 class="storeTitle">{snapshot.storeName.trim() || "EXPENSE LOG"}</h1>
      <div class="meta">
        <span>TERM {snapshot.terminalId.trim() || "—"}</span>
        <span>DATE {today}</span>
      </div>
    </header>
    <section class="items" aria-label="Recorded expenses">
      <div class="tableHead">
        <span>ITEM</span>
        <span>AMT</span>
      </div>
      {@render receiptRows(view.items, false)}
    </section>
    <footer class="footer">
      {#if view.categoryTotals.length > 0}
        <div class="breakdown">
          <p class="breakdownTitle">CATEGORY TOTALS</p>
          {#each view.categoryTotals as [cat, catSum] (cat)}
            <div class="breakdownRow">
              <span>{categoryCode(cat)}</span>
              <span>{view.money(catSum)}</span>
            </div>
          {/each}
        </div>
      {/if}
      <div class="grand">
        <span>TOTAL · {view.count}</span>
        <strong class="totalDigits">{view.money(view.total)}</strong>
      </div>
      <div class="barcodeBlock" aria-hidden="true">
        <div class="barcode">
          <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i>
          <i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
          <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
        </div>
        <span class="barcodeLabel">{today.replaceAll("-", "")}-{view.totalCode}</span>
        <span class="barcodeLabel">THANK YOU FOR LOGGING</span>
      </div>
    </footer>
  </div>
  {@render tear("bottom")}
</article>

{#snippet tear(edge: "top" | "bottom")}
  <div class="tear" data-edge={edge === "bottom" ? "bottom" : undefined} aria-hidden="true">
    {#each Array.from({ length: 12 }) as _, index (index)}<i></i>{/each}
  </div>
{/snippet}

{#snippet receiptRows(items: readonly ExpenseItem[], editable: boolean)}
  <ul class="list">
    {#each (preview ? (items).slice(0, 7) : items) as item (item.$id)}
      <li class="row">
        <div class="itemInfo">
            <span class="itemTitle">{item.title.trim() || "Untitled item"}</span>
          <span class="itemTag">{categoryCode(item.category)} · {item.time}</span>
        </div>
          <strong class="itemCost">{view.money(item.amount)}</strong>
      </li>
    {:else}
      <li class="empty">
        <strong>NO TRANSACTIONS</strong>
        <p>Nothing printed on this roll.</p>
      </li>
    {/each}
  </ul>
{/snippet}
