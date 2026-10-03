<script lang="ts">
  import { Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import doc from "./schema";
  import Chart from "./Chart.svelte";
  import { buildFlow } from "./flow";
  import { currencies, maxBuckets, maxSources, money, share, tones } from "./money";

  const plan = $derived(doc.current);
  const flow = $derived(buildFlow(plan, 100, 100));
  const verdict = $derived(
    flow.gap >= 0
      ? `${money(flow.gap, plan.currency)} left over (${share(flow.gap, flow.income)} of income)`
      : `Over budget by ${money(-flow.gap, plan.currency)}`,
  );
  const amount = (value: string) => Math.max(0, Number.isFinite(parseFloat(value)) ? Math.round(parseFloat(value) * 100) / 100 : 0);
</script>

<main class="paper" data-slop-selection="none" aria-label="Paycheck flow">
  <header class="top">
    <input class="month" aria-label="Month" placeholder="Month" value={plan.month} maxlength="24" onchange={(event) => doc.fields.month.set(event.currentTarget.value.trim())} />
    <select aria-label="Currency" value={plan.currency} onchange={(event) => doc.fields.currency.set(event.currentTarget.value as (typeof currencies)[number])}>
      {#each currencies as code}<option value={code}>{code}</option>{/each}
    </select>
  </header>
  <p class="summary" role="status" data-over={flow.gap < 0}>
    <span>In <b>{money(flow.income, plan.currency)}</b></span>
    <span>Out <b>{money(flow.spend, plan.currency)}</b></span>
    <strong>{verdict}</strong>
  </p>

  <Chart {plan} interactive height={300} />

  <section class="sheet" aria-label="Edit amounts">
    <h2>Money in</h2>
    <ul>
      {#each plan.sources as source, i (source.$id)}
        {@const row = doc.at(source)}
        <li class="r2">
          <input aria-label={`Income ${i + 1} name`} value={source.name} maxlength="24" placeholder="Where from" onchange={(event) => row.name.set(event.currentTarget.value.trim())} />
          <input aria-label={`Income ${i + 1} amount`} type="number" inputmode="decimal" min="0" step="10" value={source.amount} onchange={(event) => row.amount.set(amount(event.currentTarget.value))} />
          <button class="x" aria-label={`Remove income ${i + 1}`} onclick={() => doc.fields.sources.remove(source.$id)}><X size={16} /></button>
        </li>
      {/each}
    </ul>
    <Button.Root class="add" disabled={plan.sources.length >= maxSources} onclick={() => doc.fields.sources.insert({ name: "New income", amount: 0 })}><Plus size={16} />Add income</Button.Root>

    <h2>Money out</h2>
    <ul>
      {#each plan.buckets as bucket, i (bucket.$id)}
        {@const row = doc.at(bucket)}
        <li class="r3">
          <input aria-label={`Spending ${i + 1} name`} value={bucket.name} maxlength="24" placeholder="Where to" onchange={(event) => row.name.set(event.currentTarget.value.trim())} />
          <input aria-label={`Spending ${i + 1} amount`} type="number" inputmode="decimal" min="0" step="10" value={bucket.amount} onchange={(event) => row.amount.set(amount(event.currentTarget.value))} />
          <select class="tone" data-tone={bucket.tone} aria-label={`Spending ${i + 1} colour`} value={bucket.tone} onchange={(event) => row.tone.set(event.currentTarget.value as (typeof tones)[number])}>
            {#each tones as tone}<option value={tone}>{tone}</option>{/each}
          </select>
          <button class="x" aria-label={`Remove spending ${i + 1}`} onclick={() => doc.fields.buckets.remove(bucket.$id)}><X size={16} /></button>
        </li>
      {/each}
    </ul>
    <Button.Root class="add" disabled={plan.buckets.length >= maxBuckets} onclick={() => doc.fields.buckets.insert({ name: "New spending", amount: 0, tone: tones[plan.buckets.length % tones.length]! })}><Plus size={16} />Add spending</Button.Root>
  </section>
</main>
