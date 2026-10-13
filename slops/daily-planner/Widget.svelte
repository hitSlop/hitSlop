<script lang="ts">
  import type { CaptureProps, WidgetProps } from "hitslop/svelte";
  import type schema from "./schema";

  let { snapshot, family }: CaptureProps<typeof schema> & WidgetProps = $props();
  const priorities = $derived(snapshot.priorities.filter((item) => item.text.trim()));
  const remaining = $derived(priorities.filter((item) => !item.done));
  const focus = $derived(remaining[0]);
  const blocks = $derived([...snapshot.blocks].sort((a, b) => a.start.localeCompare(b.start)));
  const visibleBlocks = $derived(blocks.slice(0, 2));
  const day = $derived.by(() => {
    const date = new Date(`${snapshot.date}T12:00:00`);
    if (!/^\d{4}-\d{2}-\d{2}$/.test(snapshot.date) || Number.isNaN(date.valueOf()))
      return { number: "—", label: "Choose a day", full: "Undated plan" };
    const weekday = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][date.getDay()];
    const month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][date.getMonth()];
    return { number: String(date.getDate()), label: weekday, full: `${month} ${date.getDate()}, ${date.getFullYear()}` };
  });
  function shorten(text: string, length: number) {
    const letters = Array.from(text.trim());
    return letters.length > length ? `${letters.slice(0, length - 1).join("")}…` : text.trim();
  }
</script>

<article class="dp-widget" data-family={family} aria-label="Daily Planner widget">
  <header class="dp-date">
    <div><p class="dp-label">Daily planner</p><h1>{day.label}</h1><p class="dp-full-date">{day.full}</p></div>
    <strong class="dp-day">{day.number}</strong>
  </header>
  <div class="dp-content">
    <section class="dp-priorities">
      <h2>Make room for</h2>
      {#if family === "large"}
        {#each priorities.slice(0, 3) as priority, index (priority.$id)}
          <div class="dp-priority" data-done={priority.done}>
            <span class="dp-mark">{priority.done ? "✓" : index + 1}</span>
            <span>{shorten(priority.text, 45)}</span>
          </div>
        {:else}<p class="dp-empty">A little space to plan.</p>{/each}
      {:else}
        <p class="dp-focus">{focus ? shorten(focus.text, family === "small" ? 47 : 62) : priorities.length ? "Priorities complete." : "A little space to plan."}</p>
      {/if}
      <p class="dp-count">{remaining.length} {remaining.length === 1 ? "priority" : "priorities"} left</p>
    </section>
    {#if family !== "small"}
      <section class="dp-schedule">
        <h2>On the plan <span>{blocks.length} blocks</span></h2>
        {#each visibleBlocks as block (block.$id)}
          <div class="dp-block" data-kind={block.kind}>
            <span class="dp-time">{block.start}–{block.end}</span>
            <p>{shorten(block.title || "Untitled block", family === "large" ? 42 : 27)}</p>
          </div>
        {:else}<p class="dp-empty">No time blocks yet.</p>{/each}
        {#if blocks.length > visibleBlocks.length}<p class="dp-more">+{blocks.length - visibleBlocks.length} more in your day</p>{/if}
      </section>
    {/if}
  </div>
</article>

<style>
  .dp-widget { width: 100%; height: 100%; overflow: hidden; background: var(--slop-surface); color: var(--slop-ink); font-family: "Geist", sans-serif; display: flex; flex-direction: column; }
  .dp-date { display: flex; justify-content: space-between; align-items: center; padding: 13px 16px; background: var(--slop-teal); color: var(--slop-onAccent); flex-shrink: 0; }
  .dp-label { font-size: 8px; text-transform: uppercase; letter-spacing: 1.5px; margin: 0 0 4px; color: var(--slop-tealLight); }
  h1 { margin: 0; font-size: 22px; line-height: 1.1; font-weight: 700; }
  .dp-full-date { margin: 4px 0 0; font-size: 10px; color: var(--slop-tealLight); }
  .dp-day { font-size: 44px; line-height: 1; font-weight: 400; color: var(--slop-accent); }
  .dp-content { padding: 13px 16px; display: flex; flex-direction: column; gap: 13px; min-height: 0; }
  h2 { margin: 0 0 7px; font-size: 9px; line-height: 12px; text-transform: uppercase; letter-spacing: 1px; color: var(--slop-muted); display: flex; justify-content: space-between; }
  h2 span { font-size: 9px; letter-spacing: 0; text-transform: none; }
  .dp-priority { display: flex; gap: 8px; align-items: center; margin-bottom: 7px; font-size: 12px; line-height: 16px; }
  .dp-mark { width: 17px; height: 17px; flex-shrink: 0; border: 1px solid var(--slop-rule); border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 10px; }
  .dp-priority[data-done="true"] { color: var(--slop-muted); text-decoration: line-through; }
  .dp-focus { margin: 0; font-size: 14px; line-height: 18px; font-weight: 600; overflow-wrap: anywhere; }
  .dp-count, .dp-more { margin: 5px 0 0; font-size: 10px; line-height: 13px; color: var(--slop-muted); }
  .dp-block { border-left: 3px solid var(--slop-teal); border-radius: 3px; background: var(--slop-panel); padding: 4px 8px; margin-bottom: 5px; }
  .dp-block[data-kind="meeting"] { border-color: var(--slop-accent); }
  .dp-block[data-kind="break"] { background: var(--slop-mint); }
  .dp-block[data-kind="personal"] { background: var(--slop-lavender); }
  .dp-time { display: block; font-size: 9px; line-height: 12px; color: var(--slop-muted); }
  .dp-block p { margin: 1px 0 0; font-size: 11px; line-height: 14px; }
  .dp-empty { margin: 0; font-size: 12px; line-height: 16px; color: var(--slop-muted); }
  [data-family="small"] .dp-date { padding: 11px 12px; }
  [data-family="small"] h1 { font-size: 17px; }
  [data-family="small"] .dp-day { font-size: 30px; }
  [data-family="small"] .dp-content { padding: 10px 12px; }
  [data-family="small"] h2 { margin-bottom: 5px; }
  [data-family="small"] .dp-focus { font-size: 12px; line-height: 15px; max-height: 45px; overflow: hidden; }
  [data-family="medium"] .dp-date { padding: 9px 14px; }
  [data-family="medium"] .dp-label { display: none; }
  [data-family="medium"] h1 { font-size: 19px; }
  [data-family="medium"] .dp-day { font-size: 35px; }
  [data-family="medium"] .dp-content { flex-direction: row; gap: 14px; padding: 10px 14px; }
  [data-family="medium"] .dp-priorities { width: 135px; flex-shrink: 0; }
  [data-family="medium"] .dp-schedule { flex: 1; min-width: 0; }
  [data-family="medium"] .dp-focus { font-size: 12px; line-height: 15px; max-height: 45px; overflow: hidden; }
  [data-family="medium"] .dp-block { padding: 3px 6px; margin-bottom: 4px; }
  [data-family="medium"] .dp-block p { font-size: 10px; line-height: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  [data-family="medium"] .dp-more { display: none; }
</style>
