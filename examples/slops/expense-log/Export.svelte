<script lang="ts">
  import doc, { type ExpenseItem } from "./schema";
  import { CATEGORIES, categoryCode, formatAmount, formatDate, today } from "./shared";

  const total = $derived(doc.current.items.reduce((sum, item) => sum + Number(item.amount || 0), 0));

  const categoryTotals = $derived.by(() => {
    const map = new Map<string, number>();
    for (const item of doc.current.items) {
      map.set(item.category, (map.get(item.category) ?? 0) + Number(item.amount || 0));
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1]);
  });

  const symbol = $derived(doc.current.currency.trim() || "$");
  const formatMoney = (amount: number) => formatAmount(amount, symbol);
</script>

<article class="exportShell" aria-label="Exported expense receipt">
  {@render tear("top")}
  <div class="paper">
    <header class="masthead">
      <p class="stamp">*** RECEIPT ***</p>
      <h1 class="storeTitle">{doc.current.storeName.trim() || "EXPENSE LOG"}</h1>
      <div class="meta">
        <span>TERM {doc.current.terminalId.trim() || "—"}</span>
        <span>DATE {today}</span>
      </div>
    </header>
    <section class="items" aria-label="Recorded expenses">
      <div class="tableHead">
        <span>ITEM</span>
        <span>AMT</span>
      </div>
      {@render receiptRows(doc.current.items, false)}
    </section>
    <footer class="footer">
      {#if categoryTotals.length > 0}
        <div class="breakdown">
          <p class="breakdownTitle">CATEGORY TOTALS</p>
          {#each categoryTotals as [cat, catSum] (cat)}
            <div class="breakdownRow">
              <span>{categoryCode(cat)}</span>
              <span>{formatMoney(catSum)}</span>
            </div>
          {/each}
        </div>
      {/if}
      <div class="grand">
        <span>TOTAL · {doc.current.items.length}</span>
        <strong class="totalDigits">{formatMoney(total)}</strong>
      </div>
      <div class="barcodeBlock" aria-hidden="true">
        <div class="barcode">
          <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i>
          <i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
          <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
        </div>
        <span class="barcodeLabel">{today.replaceAll("-", "")}-{String(Math.round(total * 100)).padStart(6, "0")}</span>
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
    {#each items as item (item.$id)}
      <li class="row">
        <div class="itemInfo">
            <span class="itemTitle">{item.title.trim() || "Untitled item"}</span>
          <span class="itemTag">{categoryCode(item.category)} · {item.time}</span>
        </div>
          <strong class="itemCost">{formatMoney(item.amount)}</strong>
      </li>
    {:else}
      <li class="empty">
        <strong>NO TRANSACTIONS</strong>
        <p>Nothing printed on this roll.</p>
      </li>
    {/each}
  </ul>
{/snippet}
