<script lang="ts">
import { ui } from "./ui.svelte";
import { useDocument } from "@hitslop/document/svelte";
import Check from "@lucide/svelte/icons/check";
import schema, { type Habit } from "./schema";
import { calendarWeeks, completedDays, labelDay, localDate, shortDay, streak, weekdays } from "./calendar";
const doc = useDocument(schema);
const { days, labels } = $derived(calendarWeeks(ui.today));
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
          data-future={day > ui.today ? "" : undefined} aria-label={`${labelDay(day)}: ${habit.checkins[day] ? "complete" : "not complete"}`}>
          {#if habit.checkins[day]}<Check size={14} strokeWidth={2.5} />{/if}
        </span>
      {/each}
    </div>
  </div>
{/snippet}
<article class="habit-export">
      <header><p class="habit-eyebrow">Daily practice / 12 weeks</p><h1>Keep the thread</h1><p>{shortDay(days[0]!)} — {shortDay(ui.today)} · {localDate(ui.today).getFullYear()}</p></header>
      {#each doc.current.habits as habit (habit.$id)}
        <section class="habit-export-record">
          <div class="habit-detail"><h2>{habit.name || "Untitled habit"}</h2><p>{completedDays(habit.checkins, days, ui.today)} check-ins · {streak(habit.checkins, ui.today)} day streak</p></div>
          {@render calendar(habit)}
        </section>
      {:else}<p>No habits yet.</p>{/each}
      <footer>Small things, repeated. · Habit Heatmap</footer>
    </article>
