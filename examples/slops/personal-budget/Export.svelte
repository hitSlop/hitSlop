<script lang="ts">
  import doc from "./schema";
  import { categoryPercent, clamp, formatCurrency, money } from "./shared";

  const data = $derived(doc.current);

  const totalSpent = $derived(doc.current.categories.reduce((sum, cat) => sum + money(cat.spent), 0));

  const remaining = $derived(doc.current.income - totalSpent);

  const remainingRatio = $derived(doc.current.income > 0 ? clamp(Math.max(0, remaining) / doc.current.income * 100, 0, 100) : 0);

  const overspent = $derived(remaining < 0);
</script>

<article class="exportChassis" aria-label="Exported personal budget">
  <section class="ledger" aria-label="Monthly budget ledger">
    <div class="ledgerHead">
      <h1 class="month">{data.month.trim() || "Monthly budget"}</h1>
      <span class="pill">Ledger</span>
    </div>

    <div class="hero" data-over={overspent}>
      <span class="heroLabel">Remaining funds</span>
      <span class="heroValue">{formatCurrency(remaining)}</span>
      <span class="heroSub">{overspent ? "over monthly income" : "left of monthly income"}</span>
      <div class="remainingTrack" aria-hidden="true">
        <div class="remainingFill" style:transform={`scaleX(${remainingRatio / 100})`}></div>
      </div>
    </div>

    <div class="stats">
      <div class="chip">
        <span class="chipLabel">Income</span>
        <span class="chipValue">{formatCurrency(money(data.income))}</span>
      </div>
      <div class="chip">
        <span class="chipLabel">Expenses</span>
        <span class="chipValue">{formatCurrency(totalSpent)}</span>
      </div>
      <div class="chip">
        <span class="chipLabel">Savings</span>
        <span class="chipValue">{formatCurrency(money(data.savings))}</span>
      </div>
    </div>

    <div class="categories">
      <div class="catHead"><span class="catTitle">Categories</span></div>
      <ul class="catList">
        {#each data.categories as cat (cat.$id)}
          {@const percent = categoryPercent(cat)}
          {@const isOver = money(cat.spent) > money(cat.allocated)}
          <li class="catRow">
            <div class="catMeta">
              <span class="catSelect" data-active="true"><span class="catDot"></span></span>
              <span class="catName">{cat.name.trim() || "Untitled"}</span>
              <div class="catFigures">
                <span class="spent">{formatCurrency(money(cat.spent))}</span>
                <span class="divider">/</span>
                <span class="limit">{formatCurrency(money(cat.allocated))}</span>
              </div>
            </div>
            <div class="catTrack" aria-hidden="true">
              <div class="catFill" data-over={isOver} style:transform={`scaleX(${percent / 100})`}></div>
            </div>
          </li>
        {:else}
          <li class="empty"><strong>No categories.</strong></li>
        {/each}
      </ul>
    </div>
  </section>

  <section class="calc" aria-label="Calculator readout">
    <div class="lcd">
      <div class="lcdHistory">{data.month.trim() || "Budget"}</div>
      <div class="lcdDigits">{formatCurrency(remaining)}</div>
    </div>
    <div class="keys" aria-hidden="true">
      <span class="key" data-kind="fn">MC</span>
      <span class="key" data-kind="fn">MR</span>
      <span class="key" data-kind="fn">M+</span>
      <span class="key" data-kind="fn">M-</span>
      <span class="key">7</span><span class="key">8</span><span class="key">9</span><span class="key" data-kind="op">÷</span>
      <span class="key">4</span><span class="key">5</span><span class="key">6</span><span class="key" data-kind="op">×</span>
      <span class="key">1</span><span class="key">2</span><span class="key">3</span><span class="key" data-kind="op">−</span>
      <span class="key" data-kind="fn">C</span><span class="key">0</span><span class="key">.</span><span class="key" data-kind="op">+</span>
      <span class="key" data-kind="accent">=</span>
    </div>
  </section>
</article>
