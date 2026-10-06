<script lang="ts">
  import doc from "./schema";
  import { ui } from "./ui.svelte";
  import Repeat2 from "@lucide/svelte/icons/repeat-2";
  import { cadenceLabel, dateLabel, localDate, monthlyAmount, renewalState } from "./shared";
  import { type Subscription } from "./schema";
  import CategoryMark from "./CategoryMark.svelte";

  const active = $derived(doc.current.subscriptions.filter((item) => item.active));
  const visible = $derived([...doc.current.subscriptions]
  .filter((item) => ui.view === "all" || item.active)
  .sort((a, b) => Number(b.active) - Number(a.active) || a.nextRenewal.localeCompare(b.nextRenewal) || a.name.localeCompare(b.name)));
  const pace = $derived(active.reduce((total, item) => total + monthlyAmount(item), 0));

  function money(value: number): string {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: doc.current.currency }).format(value);
  }
</script>

<article class="exportLedger" aria-label="Exported subscription ledger">
  <header class="masthead">
    <div>
      <h1>Subscriptions<span class="headingLoop" aria-hidden="true"><Repeat2 size={24} /></span></h1>
    </div>
    <div class="currencyField">
      <span>Currency</span>
      <strong class="currencyTrigger">{doc.current.currency}</strong>
    </div>
  </header>
  <section class="totals" aria-label="Active subscription totals">
    <div><span>Monthly total</span><strong>{money(pace)}</strong></div>
    <div><span>Yearly estimate</span><b>{money(pace * 12)}</b></div>
    <div class="totalNote"><span>{active.length} active service{active.length === 1 ? "" : "s"}</span></div>
  </section>
  <section class="services" aria-label={ui.view === "active" ? "Active services" : "All services"}>
    <div class="listHead">
      <h2>{ui.view === "active" ? "Active services" : "All services"}</h2>
    </div>
    {#if visible.length}
      {@render serviceRows(visible, false)}
    {:else}
      <div class="empty">
        <strong>No recurring costs yet.</strong>
        <p>Add a service you want to keep an eye on.</p>
      </div>
    {/if}
  </section>
  <footer class="footer">
    <span>{active.length} active service{active.length === 1 ? "" : "s"}</span>
    <span>{doc.current.subscriptions.length} on the ledger</span>
  </footer>
</article>

{#snippet serviceRows(items: readonly Subscription[], editable: boolean)}
  <ol class="serviceList">
    {#each items as item (item.$id)}
      <li class="row" data-paused={!item.active}>

          <div class="serviceCopy">
            <CategoryMark category={item.category} /><span class="serviceText"><strong>{item.name}</strong>
            <small>{item.category}{item.note ? ` · ${item.note}` : ""}</small></span>
          </div>

        <time data-state={editable && item.active ? renewalState(item.nextRenewal) : "normal"}>
          <span>{!item.active ? "Paused" : editable && renewalState(item.nextRenewal) === "overdue" ? "Date passed" : "Renews"}</span>
          <b>{dateLabel(item.nextRenewal)}</b>
        </time>
        <output>
          <strong>{money(item.amount)}</strong>
          <small>/{cadenceLabel(item.cadence)}</small>
        </output>

      </li>
    {/each}
  </ol>
{/snippet}
