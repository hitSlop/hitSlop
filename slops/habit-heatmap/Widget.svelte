<script lang="ts">
  import type { CaptureProps, WidgetProps } from "hitslop/svelte";
  import type schema from "./schema";

  let { snapshot, family }: CaptureProps<typeof schema> & WidgetProps = $props();
  const now = new Date();
  const dayKey = (date: Date) => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  const today = dayKey(now);
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  // Calendar arithmetic at local noon stays correct across daylight-saving changes.
  const days = $derived(Array.from({ length: family === "large" ? 14 : 7 }, (_, index) => dayKey(new Date(now.getFullYear(), now.getMonth(), now.getDate() + index - (family === "large" ? 13 : 6), 12))));
  const completed = $derived(snapshot.habits.filter(habit => habit.checkins[today] > 0).length);
  const habits = $derived(snapshot.habits.slice(0, family === "large" ? 4 : family === "medium" ? 2 : 1));
  const dateLabel = (day: string) => `${months[Number(day.slice(5, 7)) - 1]} ${Number(day.slice(8))}`;
  function name(value: string) {
    const characters = Array.from(value.trim() || "Untitled habit");
    const limit = family === "small" ? 19 : 33;
    return characters.length > limit ? `${characters.slice(0, limit - 1).join("")}…` : characters.join("");
  }
</script>

<main class="habit-widget" data-family={family}>
  <header class="habit-widget-header">
    <div><p class="habit-widget-label">Daily practice</p><h1>{dateLabel(today)}</h1></div>
    <p class="habit-widget-score"><strong>{completed}<span>/{snapshot.habits.length}</span></strong><span>done today</span></p>
  </header>
  <section class="habit-widget-paper">
    {#each habits as habit (habit.$id)}
      <div class="habit-widget-record" data-tone={habit.color}>
        <div class="habit-widget-name"><h2>{name(habit.name)}</h2><span>{habit.checkins[today] ? "Done" : "To do"}</span></div>
        <div class="habit-widget-days" aria-label={`${habit.name || "Untitled habit"}, recent check-ins`}>
          {#each days as day}
            <span class="habit-widget-cell" data-complete={habit.checkins[day] > 0} data-today={day === today} aria-label={`${day}: ${habit.checkins[day] > 0 ? "complete" : "not complete"}`}>
              {#if habit.checkins[day] > 0}<svg width="10" height="10" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="m2 6 2.5 2.5L10 3" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>{/if}
            </span>
          {/each}
        </div>
      </div>
    {:else}
      <p class="habit-widget-empty">Small things, repeated.<br/>Add your first habit.</p>
    {/each}
    {#if habits.length}<p class="habit-widget-range">{dateLabel(days[0]!)} — {dateLabel(today)}</p>{/if}
  </section>
  {#if family === "large"}<footer class="habit-widget-footer">{snapshot.habits.length > habits.length ? `+ ${snapshot.habits.length - habits.length} more habits` : "Keep the thread."}</footer>{/if}
</main>

<style>
  .habit-widget { box-sizing: border-box; display: flex; flex-direction: column; gap: 11px; width: 100%; height: 100%; padding: 16px; overflow: hidden; background: var(--slop-shell); color: var(--slop-paper); font-family: sans-serif; }
  .habit-widget-header { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .habit-widget-label { margin: 0 0 4px; color: var(--slop-coral); font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: .06em; }
  .habit-widget-header h1 { margin: 0; font-size: 25px; font-weight: 650; letter-spacing: -.04em; line-height: 1; }
  .habit-widget-score { display: flex; flex-direction: column; align-items: flex-end; flex-shrink: 0; margin: 0; }
  .habit-widget-score strong { font-size: 31px; font-weight: 650; line-height: 1; letter-spacing: -.04em; color: var(--slop-butter); }
  .habit-widget-score strong span { font-size: 18px; }
  .habit-widget-score > span { margin-top: 3px; font-size: 10px; }
  .habit-widget-paper { flex: 1; min-height: 0; padding: 14px; border-radius: 18px 18px 6px 18px; background: var(--slop-paper); color: var(--slop-ink); }
  .habit-widget-record + .habit-widget-record { margin-top: 13px; }
  .habit-widget-name { display: flex; align-items: center; gap: 8px; justify-content: space-between; margin-bottom: 7px; }
  .habit-widget-name h2 { margin: 0; min-width: 0; font-size: 13px; line-height: 1.1; font-weight: 650; }
  .habit-widget-name > span { flex-shrink: 0; font-size: 9px; color: var(--slop-muted); }
  .habit-widget-days { display: flex; gap: 4px; }
  .habit-widget-cell { display: flex; flex: 1; justify-content: center; align-items: center; height: 18px; border: 1px solid transparent; border-radius: 3px; background: color-mix(in srgb, var(--slop-ink) 9%, var(--slop-paper)); }
  .habit-widget-cell[data-complete="true"] { background: var(--habit-tone); }
  .habit-widget-cell[data-today="true"] { border-color: var(--slop-muted); }
  .habit-widget-range { margin: 10px 0 0; color: var(--slop-muted); font-size: 9px; }
  .habit-widget-empty { margin: 5px 0; font-size: 13px; line-height: 1.5; }
  .habit-widget-footer { font-size: 11px; }
  [data-family="small"].habit-widget { padding: 12px; gap: 10px; }
  [data-family="small"] .habit-widget-label { font-size: 8px; }
  [data-family="small"] .habit-widget-header h1 { font-size: 20px; }
  [data-family="small"] .habit-widget-score strong { font-size: 24px; }
  [data-family="small"] .habit-widget-score strong span { font-size: 14px; }
  [data-family="small"] .habit-widget-score > span { font-size: 8px; }
  [data-family="small"] .habit-widget-paper { padding: 10px; }
  [data-family="small"] .habit-widget-name h2 { font-size: 11px; }
  [data-family="small"] .habit-widget-name > span { display: none; }
  [data-family="small"] .habit-widget-days { gap: 3px; }
  [data-family="small"] .habit-widget-cell { height: 15px; }
  [data-family="small"] .habit-widget-range { font-size: 8px; }
  [data-family="medium"].habit-widget { padding: 12px; gap: 6px; }
  [data-family="medium"] .habit-widget-header h1 { font-size: 22px; }
  [data-family="medium"] .habit-widget-label { font-size: 8px; }
  [data-family="medium"] .habit-widget-score strong { font-size: 26px; }
  [data-family="medium"] .habit-widget-paper { padding: 8px 12px; }
  [data-family="medium"] .habit-widget-record + .habit-widget-record { margin-top: 6px; }
  [data-family="medium"] .habit-widget-name { margin-bottom: 4px; }
  [data-family="medium"] .habit-widget-name h2 { font-size: 11px; }
  [data-family="medium"] .habit-widget-cell { height: 12px; }
  [data-family="medium"] .habit-widget-range { margin-top: 5px; font-size: 8px; }
</style>
