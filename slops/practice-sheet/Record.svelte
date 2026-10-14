<script lang="ts">
  // The practice record card: streak, the last fourteen days, and totals.
  import { shortDay, type practiceView } from "./practice";
  let { view }: { view: ReturnType<typeof practiceView> } = $props();
  const most = $derived(Math.max(30, ...view.fortnight.map(day => day.minutes)));
</script>

<div class="ps-record-top">
  <p class="ps-streak">
    <strong>{view.streak}</strong>
    <span>{view.streak === 1 ? "day" : "days"} in a row</span>
  </p>
  <dl class="ps-totals">
    <div><dt>Best run</dt><dd>{view.best} {view.best === 1 ? "day" : "days"}</dd></div>
    <div><dt>All time</dt><dd>{view.totalMinutes} min</dd></div>
  </dl>
</div>
<ol class="ps-fortnight" aria-label="Minutes practised over the last 14 days">
  {#each view.fortnight as day (day.day)}
    <li data-today={day.today} data-done={day.minutes > 0} title={`${shortDay(day.day)}: ${day.minutes} min`}>
      <span class="ps-cell" style:--fill={day.minutes ? 0.35 + 0.65 * Math.min(1, day.minutes / most) : 0}>
        <span class="ps-sr">{shortDay(day.day)}: {day.minutes ? `${day.minutes} minutes` : "no practice"}</span>
      </span>
      <span class="ps-weekday" aria-hidden="true">{day.weekday}</span>
    </li>
  {/each}
</ol>
<p class="ps-last">
  {#if view.lastSession}
    Last: {shortDay(view.lastSession.at)} · {view.lastSession.minutes} min at {view.lastSession.tempo} bpm
  {:else}
    No sessions logged yet.
  {/if}
</p>
