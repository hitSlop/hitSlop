<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Progress, Button } from "bits-ui";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import { capture } from "@hitslop/document/capture";
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import schema, { type BudgetCategory } from "./schema";

  const FILL_MS = 350;
  const doc = useDocument(schema);

  let selectedCategoryId = $state<string | undefined>(undefined);
  let calcDisplay = $state("0");
  let calcPrev = $state<number | null>(null);
  let calcOp = $state<string | null>(null);
  let waitingForOperand = $state(false);
  let calcMemory = $state(0);
  let calcHistory = $state("");

  const totalSpent = $derived(doc.current.categories.reduce((sum, cat) => sum + money(cat.spent), 0));
  const remaining = $derived(doc.current.income - totalSpent);
  const remainingRatio = $derived(doc.current.income > 0 ? clamp(Math.max(0, remaining) / doc.current.income * 100, 0, 100) : 0);
  const selectedCategory = $derived(
    doc.current.categories.find((cat) => cat.$id === selectedCategoryId) ?? doc.current.categories[1] ?? doc.current.categories[0],
  );
  const overspent = $derived(remaining < 0);
  const routeAmount = $derived(parseFloat(calcDisplay) || 0);

  const remainingMotion = new Tween(untrack(() => remainingRatio), { duration: FILL_MS, easing: cubicOut });
  const categoryMotion = new Map<string, Tween<number>>();
  let initialized = false;

  $effect.pre(() => {
    for (const cat of doc.current.categories) fillTween(cat);
  });
  $effect(() => {
    if (selectedCategoryId === undefined) {
      const initial = doc.current.categories[1] ?? doc.current.categories[0];
      if (initial) selectedCategoryId = initial.$id;
    }
    const instant = !initialized || prefersReducedMotion.current;
    const duration = instant ? 0 : FILL_MS;
    void remainingMotion.set(remainingRatio, { duration, delay: 0 });
    for (const cat of doc.current.categories) {
      void fillTween(cat).set(categoryPercent(cat), { duration, delay: 0 });
    }
    initialized = true;
  });

  onMount(() => capture.onPrepare(async () => {
    await remainingMotion.set(remainingRatio, { duration: 0, delay: 0 });
    await Promise.all(doc.current.categories.map((cat) => fillTween(cat).set(categoryPercent(cat), { duration: 0, delay: 0 })));
    await tick();
  }));

  onDestroy(() => {
    void remainingMotion.set(remainingMotion.target, { duration: 0, delay: 0 });
    for (const tween of categoryMotion.values()) void tween.set(tween.target, { duration: 0, delay: 0 });
  });

  function money(value: number): number {
    return Number.isFinite(value) ? value : 0;
  }
  function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
  }
  function roundCents(value: number): number {
    return Math.round(value * 100) / 100;
  }
  function categoryPercent(cat: BudgetCategory): number {
    const allocated = money(cat.allocated);
    const spent = money(cat.spent);
    if (allocated <= 0) return spent > 0 ? 100 : 0;
    return clamp((spent / allocated) * 100, 0, 100);
  }
  function fillTween(cat: BudgetCategory): Tween<number> {
    let tween = categoryMotion.get(cat.$id);
    if (!tween) {
      tween = new Tween(untrack(() => categoryPercent(cat)), { duration: FILL_MS, easing: cubicOut });
      categoryMotion.set(cat.$id, tween);
    }
    return tween;
  }
  function fillWidth(cat: BudgetCategory): number {
    return fillTween(cat).current;
  }
  function formatCurrency(num: number): string {
    return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2 }).format(num);
  }
  function addCategory() {
    const { id } = doc.fields.categories.insert({ name: "New Category", allocated: 200, spent: 0 });
    selectedCategoryId = id;
  }
  function removeCategory(id: string) {
    const next = doc.current.categories.find((cat) => cat.$id !== id);
    doc.fields.categories.remove(id);
    categoryMotion.delete(id);
    if (selectedCategory?.$id === id) selectedCategoryId = next?.$id;
  }
  function inputDigit(digit: string) {
    if (waitingForOperand) {
      calcDisplay = digit;
      waitingForOperand = false;
    } else {
      calcDisplay = calcDisplay === "0" ? digit : calcDisplay + digit;
    }
  }
  function inputDot() {
    if (waitingForOperand) {
      calcDisplay = "0.";
      waitingForOperand = false;
      return;
    }
    if (!calcDisplay.includes(".")) calcDisplay += ".";
  }
  function clearCalc() {
    calcDisplay = "0";
    calcPrev = null;
    calcOp = null;
    waitingForOperand = false;
    calcHistory = "";
  }
  function compute(first: number, second: number, op: string): number {
    switch (op) {
      case "+": return first + second;
      case "-": return first - second;
      case "*": return first * second;
      case "/": return second !== 0 ? first / second : 0;
      default: return second;
    }
  }
  function handleOp(nextOp: string) {
    const inputValue = parseFloat(calcDisplay);
    if (calcPrev === null) {
      calcPrev = inputValue;
    } else if (calcOp && !waitingForOperand) {
      const result = compute(calcPrev || 0, inputValue, calcOp);
      calcDisplay = String(result);
      calcPrev = result;
    }
    waitingForOperand = true;
    calcOp = nextOp;
    calcHistory = `${calcPrev} ${nextOp}`;
  }
  function handleEquals() {
    const inputValue = parseFloat(calcDisplay);
    if (calcOp && calcPrev !== null) {
      const result = compute(calcPrev, inputValue, calcOp);
      calcHistory = `${calcPrev} ${calcOp} ${inputValue} =`;
      calcDisplay = String(roundCents(result));
      calcPrev = null;
      calcOp = null;
      waitingForOperand = true;
    }
  }
  function handleMemory(action: "MC" | "MR" | "M+" | "M-") {
    const val = parseFloat(calcDisplay) || 0;
    if (action === "MC") calcMemory = 0;
    if (action === "MR") {
      calcDisplay = String(calcMemory);
      waitingForOperand = true;
    }
    if (action === "M+") calcMemory += val;
    if (action === "M-") calcMemory -= val;
  }
  function routeToCategory() {
    const row = selectedCategory;
    const amount = parseFloat(calcDisplay);
    if (!row || Number.isNaN(amount) || amount <= 0) return;
    const name = row.name;
    doc.at(row).spent.set(roundCents(money(row.spent) + amount));
    calcHistory = `Logged +${formatCurrency(amount)} to ${name}`;
  }
  function handleKeydown(event: KeyboardEvent) {
    const tag = (event.target as HTMLElement)?.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea") return;
    if (event.key >= "0" && event.key <= "9") inputDigit(event.key);
    else if (event.key === ".") inputDot();
    else if (["+", "-", "*", "/"].includes(event.key)) handleOp(event.key);
    else if (event.key === "Enter" || event.key === "=") {
      event.preventDefault();
      handleEquals();
    } else if (event.key === "Escape" || event.key === "c" || event.key === "C") clearCalc();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<Slop>
  <main class="canvas" data-slop-selection="none" aria-label="Personal budget">
    <div class="chassis">
      <section class="ledger" aria-label="Monthly budget ledger">
        <div class="ledgerHead">
          <input class="month" aria-label="Budget month" use:bindValue={doc.fields.month} />
          <span class="pill">Ledger</span>
        </div>

        <div class="hero" data-over={overspent}>
          <span class="heroLabel">Remaining funds</span>
          <span class="heroValue" aria-live="polite">{formatCurrency(remaining)}</span>
          <span class="heroSub">{overspent ? "over monthly income" : "left of monthly income"}</span>
          <Progress.Root value={remainingRatio} max={100} class="remainingTrack" aria-label="Remaining funds of monthly income">
            <div class="remainingFill" style:transform={`scaleX(${remainingMotion.current / 100})`} aria-hidden="true"></div>
          </Progress.Root>
        </div>

        <div class="stats">
          <label class="chip">
            <span class="chipLabel">Income</span>
            <input class="chipValue" type="number" min="0" step="0.01" aria-label="Monthly income" use:bindValue={doc.fields.income} />
          </label>
          <div class="chip">
            <span class="chipLabel">Expenses</span>
            <span class="chipValue">{formatCurrency(totalSpent)}</span>
          </div>
          <label class="chip">
            <span class="chipLabel">Savings</span>
            <input class="chipValue" type="number" min="0" step="0.01" aria-label="Savings target" use:bindValue={doc.fields.savings} />
          </label>
        </div>

        <div class="categories">
          <div class="catHead">
            <span class="catTitle">Categories</span>
            <Button.Root type="button" class="iconBtn" data-slop-export="hide" aria-label="Add spending category" onclick={addCategory}>
              <Plus size={14} />
            </Button.Root>
          </div>
          <ul class="catList">
            {#each doc.current.categories as cat (cat.$id)}
              {@const percent = categoryPercent(cat)}
              {@const isOver = money(cat.spent) > money(cat.allocated)}
              <li class="catRow" data-selected={selectedCategory?.$id === cat.$id}>
                <div class="catMeta">
                  <button type="button" class="catSelect" data-active={selectedCategory?.$id === cat.$id} aria-label="Select {cat.name} for calculator" aria-pressed={selectedCategory?.$id === cat.$id} onclick={() => (selectedCategoryId = cat.$id)}>
                    <span class="catDot"></span>
                  </button>
                  <input class="catName" aria-label="Category name" use:bindText={doc.at(cat).name} onfocus={() => (selectedCategoryId = cat.$id)} />
                  <div class="catFigures">
                    <span class="spent">{formatCurrency(money(cat.spent))}</span>
                    <span class="divider">/</span>
                    <input class="allocated" type="number" min="0" step="0.01" aria-label="{cat.name} allocated amount" use:bindValue={doc.at(cat).allocated} onclick={(event) => event.stopPropagation()} />
                    <Button.Root type="button" class="deleteCat" data-slop-export="hide" aria-label="Delete {cat.name}" onclick={() => removeCategory(cat.$id)}>
                      <Trash2 size={12} />
                    </Button.Root>
                  </div>
                </div>
                <Progress.Root value={percent} max={100} class="catTrack" aria-label="{cat.name} budget spent">
                  <div class="catFill" data-over={isOver} style:transform={`scaleX(${fillWidth(cat) / 100})`} aria-hidden="true"></div>
                </Progress.Root>
              </li>
            {:else}
              <li class="empty">
                <strong>No categories yet.</strong>
                Add a line, then punch an amount on the calculator and log it.
              </li>
            {/each}
          </ul>
        </div>
      </section>

      <section class="calc" aria-label="Pocket calculator">
        <div class="lcd">
          <div class="lcdHistory">{calcHistory}</div>
          <div class="lcdDigits">{calcDisplay}</div>
        </div>

        <div class="route" data-slop-export="hide">
          <Button.Root type="button" class="routeBtn" disabled={!selectedCategory || routeAmount <= 0} onclick={routeToCategory}>
            <span>Log +{formatCurrency(routeAmount)}</span>
            <ArrowRight size={13} />
            <span>{selectedCategory ? selectedCategory.name : "Select category"}</span>
          </Button.Root>
        </div>

        <div class="keys" aria-label="Calculator keys">
          <button type="button" class="key" data-kind="fn" onclick={() => handleMemory("MC")}>MC</button>
          <button type="button" class="key" data-kind="fn" onclick={() => handleMemory("MR")}>MR</button>
          <button type="button" class="key" data-kind="fn" onclick={() => handleMemory("M+")}>M+</button>
          <button type="button" class="key" data-kind="fn" onclick={() => handleMemory("M-")}>M-</button>
          <button type="button" class="key" onclick={() => inputDigit("7")}>7</button>
          <button type="button" class="key" onclick={() => inputDigit("8")}>8</button>
          <button type="button" class="key" onclick={() => inputDigit("9")}>9</button>
          <button type="button" class="key" data-kind="op" data-active={calcOp === "/"} onclick={() => handleOp("/")}>÷</button>
          <button type="button" class="key" onclick={() => inputDigit("4")}>4</button>
          <button type="button" class="key" onclick={() => inputDigit("5")}>5</button>
          <button type="button" class="key" onclick={() => inputDigit("6")}>6</button>
          <button type="button" class="key" data-kind="op" data-active={calcOp === "*"} onclick={() => handleOp("*")}>×</button>
          <button type="button" class="key" onclick={() => inputDigit("1")}>1</button>
          <button type="button" class="key" onclick={() => inputDigit("2")}>2</button>
          <button type="button" class="key" onclick={() => inputDigit("3")}>3</button>
          <button type="button" class="key" data-kind="op" data-active={calcOp === "-"} onclick={() => handleOp("-")}>−</button>
          <button type="button" class="key" data-kind="fn" onclick={clearCalc}>C</button>
          <button type="button" class="key" onclick={() => inputDigit("0")}>0</button>
          <button type="button" class="key" onclick={inputDot}>.</button>
          <button type="button" class="key" data-kind="op" data-active={calcOp === "+"} onclick={() => handleOp("+")}>+</button>
          <button type="button" class="key" data-kind="accent" onclick={handleEquals}>=</button>
        </div>
      </section>
    </div>
  </main>

  {#snippet exportView()}
    {@const data = doc.current}
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
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <div class="iconDevice">
        <div class="iconScreen">
          <span class="iconSymbol">$</span>
          <span class="iconDigits"></span>
        </div>
        <div class="iconSplit">
          <div class="iconLedger">
            <span class="iconBar"></span>
            <span class="iconBar"></span>
            <span class="iconBar"></span>
            <span class="iconBar"></span>
          </div>
          <div class="iconPad">
            <span class="iconKey"></span>
            <span class="iconKey"></span>
            <span class="iconKey"></span>
            <span class="iconKey"></span>
            <span class="iconKey"></span>
            <span class="iconKey"></span>
          </div>
        </div>
      </div>
    </div>
  {/snippet}
</Slop>
