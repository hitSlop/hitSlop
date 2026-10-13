<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
  import { moneyLabel } from "../_shared/format";
  import doc from "./schema";
  import { invoiceView } from "./items";

  const totals = $derived(invoiceView(doc.current));

  function money(value: number) {
    return moneyLabel(value, doc.current.currency);
  }
</script>

<article class="invoice-paper invoice-export" aria-label="Exported invoice {doc.current.number}">
  <header class="invoice-masthead">
    <div><span class="invoice-eyebrow">Invoice</span><h1 class="invoice-number">{doc.current.number}</h1></div>
    <span class="invoice-status-trigger"><span class="invoice-status-dot" data-status={doc.current.status}></span>{doc.current.status}</span>
  </header>
  <section class="invoice-meta">
    <div><span class="invoice-meta-label">Issued</span><strong>{doc.current.issued}</strong></div>
    <div><span class="invoice-meta-label">Due</span><strong>{doc.current.due}</strong></div>
    <div><span class="invoice-meta-label">Currency</span><strong>{doc.current.currency}</strong></div>
  </section>
  <section class="invoice-parties">
    <div class="invoice-party"><span class="invoice-party-label">From</span><strong>{doc.current.from.name}</strong><p>{doc.current.from.detail}</p></div>
    <div class="invoice-party"><span class="invoice-party-label">Bill to</span><strong>{doc.current.billTo.name}</strong><p>{doc.current.billTo.detail}</p></div>
  </section>
  <section class="invoice-lines">
    <div class="invoice-line-heading"><h2>Description</h2><span>Qty</span><span>Rate</span><span>Amount</span></div>
    {#each (preview ? (doc.current.items).slice(0, 5) : doc.current.items) as item (item.$id)}
      <div class="invoice-line"><span>{item.description}</span><span>{item.quantity}</span><span>{money(item.rate)}</span><span class="invoice-amount">{money(item.quantity * item.rate)}</span></div>
    {/each}
  </section>
  <section class="invoice-closing">
    <div class="invoice-notes"><span class="invoice-notes-label">Notes &amp; terms</span><p>{doc.current.notes}</p></div>
    <dl class="invoice-totals"><div><dt>Subtotal</dt><dd>{money(totals.subtotal)}</dd></div><div><dt>Tax</dt><dd>{money(totals.tax)}</dd></div><div class="invoice-grand"><dt>Total</dt><dd>{money(totals.total)}</dd></div></dl>
  </section>
</article>
