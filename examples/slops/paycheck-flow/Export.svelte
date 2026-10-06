<script lang="ts">
  import doc from "./schema";
  import Chart from "./Chart.svelte";
  import { buildFlow } from "./flow";
  import { money, share } from "./money";

  const plan = $derived(doc.current);
  const flow = $derived(buildFlow(plan, 100, 100));
  const verdict = $derived(
    flow.gap >= 0
      ? `${money(flow.gap, plan.currency)} left over (${share(flow.gap, flow.income)} of income)`
      : `Over budget by ${money(-flow.gap, plan.currency)}`,
  );
</script>

<article class="paper export" aria-label="Exported paycheck flow">
  <h1>{plan.month || "This month"}</h1>
  <p class="summary" data-over={flow.gap < 0}>
    <span>In <b>{money(flow.income, plan.currency)}</b></span>
    <span>Out <b>{money(flow.spend, plan.currency)}</b></span>
    <strong>{verdict}</strong>
  </p>
  <Chart {plan} width={528} height={320} />
</article>
