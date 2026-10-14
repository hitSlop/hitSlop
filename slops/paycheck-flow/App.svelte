<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { getDocument } from "hitslop/svelte";
  import { Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import schema from "./schema";
  import Chart from "./Chart.svelte";
  import { currencies, money, parseMajor, toMajor, tones, type Currency, type Tone } from "./money";
  import { addIncome, addSpending, planView, removeRow, setCurrency, setMonth, updateRow } from "./plan";

  const doc = getDocument(schema);
  const plan = $derived(doc.current);
  const view = $derived(planView(plan));
  const update = (id: string, change: Parameters<typeof updateRow>[3]) => doc.change((tx) => updateRow(tx, doc.current, id, change));
  const rename = (month: string) => doc.change((tx) => setMonth(tx, month));
  const switchCurrency = (code: Currency) => doc.change((tx) => setCurrency(tx, doc.current, code));
  const amountOf = (value: string) => parseMajor(value, doc.current.currency);
</script>

<main class="paper" aria-label="Paycheck flow">
  <header class="top">
    <input class="month" aria-label="Month" placeholder="Month" value={plan.month} maxlength="24" onchange={(event) => rename(event.currentTarget.value)} />
    <select aria-label="Currency" value={plan.currency} onchange={(event) => switchCurrency(event.currentTarget.value as Currency)}>
      {#each currencies as code}<option value={code}>{code}</option>{/each}
    </select>
  </header>
  <p class="summary" role="status" data-over={view.over}>
    <span>In <b>{money(view.income, plan.currency)}</b></span>
    <span>Out <b>{money(view.spend, plan.currency)}</b></span>
    <strong>{view.verdict}</strong>
  </p>

  <Chart {plan} interactive height={300} />

  <section class="sheet" aria-label="Edit amounts">
    <h2>Money in</h2>
    <ul>
      {#each plan.sources as source, i (source.$id)}
        <li class="r2">
          <input aria-label={`Income ${i + 1} name`} value={source.name} maxlength="24" placeholder="Where from" onchange={(event) => update(source.$id, { name: event.currentTarget.value })} />
          <input aria-label={`Income ${i + 1} amount`} type="number" inputmode="decimal" min="0" step="10" value={toMajor(source.amountMinor, plan.currency)} onchange={(event) => update(source.$id, { amountMinor: amountOf(event.currentTarget.value) })} />
          <button class="x" aria-label={`Remove income ${i + 1}`} onclick={() => doc.change((tx) => removeRow(tx, doc.current, source.$id))}><X size={16} /></button>
        </li>
      {/each}
    </ul>
    <Button.Root class="add" disabled={!view.canAddIncome} onclick={() => doc.change((tx) => addIncome(tx, doc.current, "New income"))}><Plus size={16} />Add income</Button.Root>

    <h2>Money out</h2>
    <ul>
      {#each plan.buckets as bucket, i (bucket.$id)}
        <li class="r3">
          <input aria-label={`Spending ${i + 1} name`} value={bucket.name} maxlength="24" placeholder="Where to" onchange={(event) => update(bucket.$id, { name: event.currentTarget.value })} />
          <input aria-label={`Spending ${i + 1} amount`} type="number" inputmode="decimal" min="0" step="10" value={toMajor(bucket.amountMinor, plan.currency)} onchange={(event) => update(bucket.$id, { amountMinor: amountOf(event.currentTarget.value) })} />
          <select class="tone" data-tone={bucket.tone} aria-label={`Spending ${i + 1} colour`} value={bucket.tone} onchange={(event) => update(bucket.$id, { tone: event.currentTarget.value as Tone })}>
            {#each tones as tone}<option value={tone}>{tone}</option>{/each}
          </select>
          <button class="x" aria-label={`Remove spending ${i + 1}`} onclick={() => doc.change((tx) => removeRow(tx, doc.current, bucket.$id))}><X size={16} /></button>
        </li>
      {/each}
    </ul>
    <Button.Root class="add" disabled={!view.canAddSpending} onclick={() => doc.change((tx) => addSpending(tx, doc.current, "New spending"))}><Plus size={16} />Add spending</Button.Root>
  </section>
</main>
