<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
  import doc from "./schema";
  import { subscriptionView, type Subscription } from "./items";
  import { cadenceLabel, dateLabel, money } from "./shared";
  import CategoryMark from "./CategoryMark.svelte";
  import Repeat2 from "@lucide/svelte/icons/repeat-2";

  // A fresh page: it cannot see the window's view, so it lists every service.
  const { rows, activeCount, totalCount, pace, yearly } = $derived(subscriptionView(doc.current, "all"));
  const currency = $derived(doc.current.currency);
</script>

<article class="exportLedger" aria-label="Exported subscription ledger">
  <header class="masthead">
    <div>
      <h1>Subscriptions<span class="headingLoop" aria-hidden="true"><Repeat2 size={24} /></span></h1>
    </div>
    <div class="currencyField">
      <span>Currency</span>
      <strong class="currencyTrigger">{currency}</strong>
    </div>
  </header>
  <section class="totals" aria-label="Active subscription totals">
    <div><span>Monthly total</span><strong>{money(pace, currency)}</strong></div>
    <div><span>Yearly estimate</span><b>{money(yearly, currency)}</b></div>
    <div class="totalNote"><span>{activeCount} active service{activeCount === 1 ? "" : "s"}</span></div>
  </section>
  <section class="services" aria-label="All services">
    <div class="listHead">
      <h2>All services</h2>
    </div>
    {@render serviceRows(rows)}
  </section>
  <footer class="footer">
    <span>{activeCount} active service{activeCount === 1 ? "" : "s"}</span>
    <span>{totalCount} on the ledger</span>
  </footer>
</article>

{#snippet serviceRows(items: readonly Subscription[])}
  {#if items.length}
    <ol class="serviceList">
      {#each (preview ? (items).slice(0, 5) : items) as item (item.$id)}
        <li class="row" data-paused={!item.active}>
          <div class="serviceCopy">
            <CategoryMark category={item.category} /><span class="serviceText"><strong>{item.name}</strong>
            <small>{item.category}{item.note ? ` · ${item.note}` : ""}</small></span>
          </div>
          <time data-state="normal">
            <span>{item.active ? "Renews" : "Paused"}</span>
            <b>{dateLabel(item.nextRenewal)}</b>
          </time>
          <output>
            <strong>{money(item.amount, currency)}</strong>
            <small>/{cadenceLabel(item.cadence)}</small>
          </output>
        </li>
      {/each}
    </ol>
  {:else}
    <div class="empty">
      <strong>No recurring costs yet.</strong>
      <p>Add a service you want to keep an eye on.</p>
    </div>
  {/if}
{/snippet}
