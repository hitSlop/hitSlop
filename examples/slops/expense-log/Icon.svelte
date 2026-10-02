<script lang="ts">
  import doc from "./schema";
  import { formatAmount, receiptLabel } from "./shared";

  const placeholders = [
  { title: "COFFEE", amount: 4.5 },
  { title: "METRO", amount: 2.9 },
  { title: "LUNCH", amount: 14.8 },
  { title: "BOOKS", amount: 22 },
];
  const symbol = $derived(doc.current.currency.trim() || "$");
  const formatMoney = (amount: number) => formatAmount(amount, symbol);
  const total = $derived(doc.current.items.reduce((sum, item) => sum + Number(item.amount || 0), 0));
  const lines = $derived(doc.current.items.slice(0, 4));
  const shown = $derived(lines.length ? lines : placeholders);
  const shownTotal = $derived(lines.length ? total : placeholders.reduce((sum, item) => sum + item.amount, 0));
</script>

<div class="iconSurface" aria-hidden="true">
  <article class="iconReceipt">
    <div class="iconTear">
      {#each Array.from({ length: 8 }) as _, index (index)}<i></i>{/each}
    </div>
    <div class="iconBody">
      <div class="iconHeader">
        <span class="iconTitle">EXPENSE LOG</span>
        <span class="iconSub">{doc.current.terminalId.trim() || "TERMINAL"}</span>
      </div>
      <div class="iconDash"></div>
      <div class="iconItems">
        {#each shown as item, index (index)}
          <div class="iconRow"><span>{receiptLabel(item.title)}</span><strong>{formatMoney(item.amount)}</strong></div>
        {/each}
      </div>
      <div class="iconDouble"></div>
      <div class="iconTotal">
        <span>TOTAL</span>
        <strong>{formatMoney(shownTotal)}</strong>
      </div>
      <div class="iconBarcode">
        <i></i><i data-wide="true"></i><i></i><i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i>
        <i></i><i data-wide="true"></i><i></i><i data-wide="true"></i><i></i><i></i>
      </div>
    </div>
  </article>
</div>
