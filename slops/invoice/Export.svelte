<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import { moneyLabel } from "../_shared/format";
  import type schema from "./schema";
  import { invoiceView } from "./items";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const totals = $derived(invoiceView(snapshot));

  function money(value: number) {
    return moneyLabel(value, snapshot.currency);
  }
</script>

<article class="invoice-paper invoice-export" aria-label="Exported invoice {snapshot.number}">
  <header class="invoice-masthead">
    <div><span class="invoice-eyebrow">Invoice</span><h1 class="invoice-number">{snapshot.number}</h1></div>
    <span class="invoice-status-trigger"><span class="invoice-status-dot" data-status={snapshot.status}></span>{snapshot.status}</span>
  </header>
  <section class="invoice-meta">
    <div><span class="invoice-meta-label">Issued</span><strong>{snapshot.issued}</strong></div>
    <div><span class="invoice-meta-label">Due</span><strong>{snapshot.due}</strong></div>
    <div><span class="invoice-meta-label">Currency</span><strong>{snapshot.currency}</strong></div>
  </section>
  <section class="invoice-parties">
    <div class="invoice-party"><span class="invoice-party-label">From</span><strong>{snapshot.from.name}</strong><p>{snapshot.from.detail}</p></div>
    <div class="invoice-party"><span class="invoice-party-label">Bill to</span><strong>{snapshot.billTo.name}</strong><p>{snapshot.billTo.detail}</p></div>
  </section>
  <section class="invoice-lines">
    <div class="invoice-line-heading"><h2>Description</h2><span>Qty</span><span>Rate</span><span>Amount</span></div>
    {#each (preview ? (snapshot.items).slice(0, 5) : snapshot.items) as item (item.$id)}
      <div class="invoice-line"><span>{item.description}</span><span>{item.quantity}</span><span>{money(item.rate)}</span><span class="invoice-amount">{money(item.quantity * item.rate)}</span></div>
    {/each}
  </section>
  <section class="invoice-closing">
    <div class="invoice-notes"><span class="invoice-notes-label">Notes &amp; terms</span><p>{snapshot.notes}</p></div>
    <dl class="invoice-totals"><div><dt>Subtotal</dt><dd>{money(totals.subtotal)}</dd></div><div><dt>Tax</dt><dd>{money(totals.tax)}</dd></div><div class="invoice-grand"><dt>Total</dt><dd>{money(totals.total)}</dd></div></dl>
  </section>
</article>
