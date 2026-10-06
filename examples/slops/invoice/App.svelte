<script lang="ts">
  import { Button, Select } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { bindText } from "@hitslop/document/svelte";
  import doc, { currencies, statuses } from "./schema";
  const statusItems = statuses.map((value) => ({ value, label: value }));
  const currencyItems = currencies.map((value) => ({ value, label: value }));
  const subtotal = $derived(doc.current.items.reduce((sum, item) => sum + item.quantity * item.rate, 0));
  const tax = $derived(subtotal * doc.current.taxPercent / 100);
  const total = $derived(subtotal + tax);

  function money(value: number) {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: doc.current.currency }).format(value);
  }

  function setStatus(value: string | undefined) {
    if (statuses.some((status) => status === value)) doc.fields.status.set(value as (typeof statuses)[number]);
  }

  function setCurrency(value: string | undefined) {
    if (currencies.some((currency) => currency === value)) doc.fields.currency.set(value as (typeof currencies)[number]);
  }

  function addLine() {
    doc.fields.items.insert({ description: "", quantity: 1, rate: 0 });
  }
</script>

  <main class="invoice-canvas">
    <article class="invoice-paper" aria-label="Invoice {doc.current.number}">
      <header class="invoice-masthead">
        <div>
          <span class="invoice-eyebrow">Invoice</span>
          <label>
            <span class="invoice-sr-only">Invoice number</span>
            <input class="invoice-number" aria-label="Invoice number" use:bindText={doc.fields.number} />
          </label>
        </div>
        <Select.Root type="single" value={doc.current.status} items={statusItems} onValueChange={setStatus}>
          <Select.Trigger class="invoice-status-trigger" aria-label="Invoice status">
            <span class="invoice-status-dot" data-status={doc.current.status}></span>
            <Select.Value placeholder="Status" />
            <ChevronDown size={13} data-slop-export="hide" strokeWidth={1.8} />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="invoice-select-content" sideOffset={6}>
              <Select.Viewport>
                {#each statusItems as item (item.value)}
                  <Select.Item value={item.value} label={item.label}>
                    {#snippet children({ selected })}{item.label}{#if selected}<Check size={13} strokeWidth={1.8} />{/if}{/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      </header>

      <section class="invoice-meta" aria-label="Invoice details">
        <label><span class="invoice-meta-label">Issued</span><input aria-label="Issue date" type="date" bind:value={doc.fields.issued.value} /></label>
        <label><span class="invoice-meta-label">Due</span><input aria-label="Due date" type="date" bind:value={doc.fields.due.value} /></label>
        <div>
          <span class="invoice-meta-label">Currency</span>
          <Select.Root type="single" value={doc.current.currency} items={currencyItems} onValueChange={setCurrency}>
            <Select.Trigger class="invoice-currency-trigger" aria-label="Invoice currency">
              <Select.Value placeholder="Currency" />
              <ChevronDown size={12} data-slop-export="hide" strokeWidth={1.8} />
            </Select.Trigger>
            <Select.Portal>
              <Select.Content class="invoice-select-content" sideOffset={6}>
                <Select.Viewport>
                  {#each currencyItems as item (item.value)}
                    <Select.Item value={item.value} label={item.label}>
                      {#snippet children({ selected })}{item.label}{#if selected}<Check size={13} strokeWidth={1.8} />{/if}{/snippet}
                    </Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
        </div>
      </section>

      <section class="invoice-parties" aria-label="Invoice parties">
        <label class="invoice-party">
          <span class="invoice-party-label">From</span>
          <input aria-label="From name" placeholder="Your name or company" use:bindText={doc.at(doc.current.from).name} />
          <textarea aria-label="From details" placeholder="Address and contact details" use:bindText={doc.at(doc.current.from).detail}></textarea>
        </label>
        <label class="invoice-party">
          <span class="invoice-party-label">Bill to</span>
          <input aria-label="Bill-to name" placeholder="Client name or company" use:bindText={doc.at(doc.current.billTo).name} />
          <textarea aria-label="Bill-to details" placeholder="Address and contact details" use:bindText={doc.at(doc.current.billTo).detail}></textarea>
        </label>
      </section>

      <section class="invoice-lines" aria-label="Line items">
        <div class="invoice-line-heading"><h2>Description</h2><span>Qty</span><span>Rate</span><span>Amount</span><span data-slop-export="hide"></span></div>
        {#each doc.current.items as item, index (item.$id)}
          <div class="invoice-line">
            <label><span class="invoice-mobile-label">Item</span><input aria-label="Description for line {index + 1}" placeholder="Service or item" use:bindText={doc.at(item).description} /></label>
            <label><span class="invoice-mobile-label">Qty</span><input aria-label="Quantity for line {index + 1}" type="number" min="0" step="0.01" bind:value={() => doc.at(item).quantity.value, (next) => { doc.at(item).quantity.value = next; }} /></label>
            <label><span class="invoice-mobile-label">Rate</span><input aria-label="Rate for line {index + 1}" type="number" min="0" step="0.01" bind:value={() => doc.at(item).rate.value, (next) => { doc.at(item).rate.value = next; }} /></label>
            <output class="invoice-amount"><span class="invoice-mobile-label">Amount</span>{money(item.quantity * item.rate)}</output>
            <button class="invoice-remove" data-slop-export="hide" aria-label="Remove line {index + 1}" onclick={() => doc.fields.items.remove(item.$id)}><Trash2 size={15} strokeWidth={1.7} /></button>
          </div>
        {/each}
        <Button.Root class="invoice-add" data-slop-export="hide" onclick={addLine}><Plus size={14} strokeWidth={1.8} /> Add line item</Button.Root>
      </section>

      <section class="invoice-closing">
        <label class="invoice-notes">
          <span class="invoice-notes-label">Notes &amp; terms</span>
          <textarea aria-label="Notes and terms" placeholder="Payment terms, delivery notes, or a thank-you" use:bindText={doc.fields.notes}></textarea>
        </label>
        <dl class="invoice-totals">
          <div><dt>Subtotal</dt><dd>{money(subtotal)}</dd></div>
          <div><dt>Tax <input aria-label="Tax percentage" type="number" min="0" max="100" step="0.1" bind:value={doc.fields.taxPercent.value} />%</dt><dd>{money(tax)}</dd></div>
          <div class="invoice-grand"><dt>Total</dt><dd>{money(total)}</dd></div>
        </dl>
      </section>
    </article>
  </main>
