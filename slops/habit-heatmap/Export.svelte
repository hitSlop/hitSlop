<script lang="ts">
  let { preview = false }: { preview?: boolean } = $props();
import Check from "@lucide/svelte/icons/check";
import doc from "./schema";
import { dayKey, heatmapView, labelDay, localDate, shortDay, weekdays } from "./calendar";
type Habit = (typeof doc.current.habits)[number];
const today = dayKey(new Date());
const view = $derived(heatmapView(doc.current, today));
const { days, labels } = $derived(view);
</script>
{#snippet calendar(habit: Habit)}
  <div class="habit-calendar" data-tone={habit.color}>
    <div class="habit-week-labels" aria-hidden="true">
      {#each labels as label}<span>{label}</span>{/each}
    </div>
    <div class="habit-weekdays" aria-hidden="true">{#each weekdays as day}<span>{day}</span>{/each}</div>
    <div class="habit-grid" role="group" aria-label={`${habit.name}: daily check-ins`}>
      {#each days as day}
        <span class="habit-cell" data-state={habit.checkins[day] ? "checked" : "unchecked"}
          data-future={day > today ? "" : undefined} aria-label={`${labelDay(day)}: ${habit.checkins[day] ? "complete" : "not complete"}`}>
          {#if habit.checkins[day]}<Check size={14} strokeWidth={2.5} />{/if}
        </span>
      {/each}
    </div>
  </div>
{/snippet}
<article class="habit-export">
      <header><p class="habit-eyebrow">Daily practice / 12 weeks</p><h1>Keep the thread</h1><p>{shortDay(days[0]!)} — {shortDay(today)} · {localDate(today).getFullYear()}</p></header>
      {#each (preview ? (view.habits).slice(0, 2) : view.habits) as { habit, streak, completed } (habit.$id)}
        <section class="habit-export-record">
          <div class="habit-detail"><h2>{habit.name || "Untitled habit"}</h2><p>{completed} check-ins · {streak} day streak</p></div>
          {@render calendar(habit)}
        </section>
      {:else}<p>No habits yet.</p>{/each}
      <footer>Small things, repeated. · Habit Heatmap</footer>
    </article>
