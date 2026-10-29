<script lang="ts">
  import type { Snippet } from "svelte";
  import Strip from "./Strip.svelte";
  import { capInScale } from "./strip";
  import { ASSUMED_EXAM_DAYS, formatDay, type Deck, type DeckView } from "./deck";

  interface Props { data: Deck; view: DeckView; stripWidth?: number; children?: Snippet }
  const { data, view, stripWidth = 428, children }: Props = $props();
  const f = $derived(view.forecast);
  // The strip fills its box, so it follows the window as it is resized.
  let boxWidth = $state(0);
  const width = $derived(boxWidth > 0 ? boxWidth - 28 : stripWidth);
  const percent = (value: number) => `${Math.round(value * 100)}%`;
  const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
  const stripTitle = $derived.by(() => {
    if (!f) return "";
    const n = view.strip.length;
    if (f.exam.daysLeft >= 0 && f.exam.daysLeft <= 14) return n === 1 ? "Until the exam: today" : `Next ${n} days, up to the exam`;
    return `Next ${n} days`;
  });
</script>

{#if view.problem}
  <section class="deck-box deck-problem" role="alert">
    <h2 class="deck-label">Can't schedule this deck</h2>
    <p>{view.problem}</p>
    <p class="deck-fine">The saved values are left as they are. Fix the scheduler settings with the command line, or reset progress.</p>
  </section>
{:else if f}
  {@const t = f.today}
  {@const reviewsLeft = Math.min(t.dueToday, t.planned)}
  <section class="deck-box deck-today" aria-labelledby="deck-today-label">
    <h2 class="deck-label" id="deck-today-label">Today</h2>
    <div class="deck-today-row">
      <span class="deck-big">{t.planned}</span>
      <div class="deck-today-side">
        <span class="deck-big-unit">{t.planned === 1 ? "card" : "cards"} today</span>
        <p class="deck-breakdown">
          <span><b>{reviewsLeft}</b> due</span>
          <span><b>{t.planned - reviewsLeft}</b> new</span>
          <span><b>{t.doneToday}</b> done</span>
        </p>
      </div>
    </div>
    <p class="deck-cap" data-cut={view.cutByCap > 0}>
      {#if view.cutByCap > 0}
        <strong>Budget full:</strong> {data.dailyMinutes} min holds {t.cap} cards, so {plural(view.cutByCap, "more card")} {view.cutByCap === 1 ? "doesn't" : "don't"} fit today.
      {:else}
        Budget {data.dailyMinutes} min ÷ {data.secondsPerCard} s a card = {t.cap} cards a day.
      {/if}
    </p>
  </section>

  {@render children?.()}

  <section class="deck-box deck-forecast" aria-labelledby="deck-forecast-label" bind:clientWidth={boxWidth}>
    <h2 class="deck-label" id="deck-forecast-label">{stripTitle}</h2>
    <Strip days={view.strip} cap={t.cap} {width} />
    <p class="deck-legend">
      <span><i class="deck-swatch" data-kind="review"></i>reviews</span>
      <span><i class="deck-swatch" data-kind="new"></i>new cards</span>
      {#if capInScale(view.strip, t.cap)}<span><i class="deck-swatch" data-kind="cap"></i>daily cap {t.cap}</span>{:else}<span>daily cap {t.cap}, above the busiest day</span>{/if}
      {#if view.strip.some(day => day.overBudget)}<span class="deck-legend-over">! budget full, cards wait</span>{/if}
      {#if view.strip.some(day => Math.round(day.reviews) > t.cap)}<span>above the cap: same-day repeats, never cut</span>{/if}
    </p>
  </section>

  <section class="deck-box deck-ready" aria-labelledby="deck-ready-label">
    <h2 class="deck-label" id="deck-ready-label">Ready by exam <em>estimate</em></h2>
    {#if f.exam.daysLeft < 0}
      <p class="deck-ready-text">The exam date, {formatDay(f.exam.date, "long")}, has passed. Set the next one in Settings.</p>
    {:else}
      <p class="deck-headline deck-ready-line">
        <span class="deck-mid">≈{percent(f.exam.expectedRecall)}</span>
        <span class="deck-big-unit">expected recall on {f.exam.daysLeft === 0 ? "exam morning" : formatDay(f.exam.date, "long")}</span>
      </p>
      <p class="deck-ready-text">
        About {Math.round(f.exam.readyCards)} of {f.exam.total} cards at your {percent(data.scheduler.requestRetention)} target{#if f.exam.unseen >= 0.5}; {Math.round(f.exam.unseen)} not studied by then{/if}.
      </p>
      <p class="deck-fine">
        {f.exam.assumed ? `No exam date set, so this assumes an exam ${ASSUMED_EXAM_DAYS} days out (${formatDay(f.exam.date)}). ` : ""}A model estimate that assumes you study the planned cards. Not a promise.
      </p>
    {/if}
  </section>
{/if}
