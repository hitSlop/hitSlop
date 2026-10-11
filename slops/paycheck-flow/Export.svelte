<script lang="ts">
  import doc from "./schema";
  import Chart from "./Chart.svelte";
  import { money } from "./money";
  import { planView } from "./plan";

  const plan = $derived(doc.current);
  const view = $derived(planView(plan));
</script>

<article class="paper export" aria-label="Exported paycheck flow">
  <h1>{plan.month || "This month"}</h1>
  <p class="summary" data-over={view.over}>
    <span>In <b>{money(view.income, plan.currency)}</b></span>
    <span>Out <b>{money(view.spend, plan.currency)}</b></span>
    <strong>{view.verdict}</strong>
  </p>
  <Chart {plan} width={528} height={320} />
</article>
