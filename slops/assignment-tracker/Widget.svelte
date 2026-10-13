<script lang="ts">
  import type { CaptureProps, WidgetProps } from "hitslop/svelte";
  import type schema from "./schema";

  let { snapshot, family, width, height }: CaptureProps<typeof schema> & WidgetProps = $props();
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  function validDate(value: string) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
    const [year, month, day] = value.split("-").map(Number);
    const date = new Date(0);
    date.setUTCFullYear(year, month - 1, day);
    return year > 0 && date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
  }
  function shortDate(value: string) {
    return `${months[Number(value.slice(5, 7)) - 1]} ${Number(value.slice(8))}`;
  }
  const active = $derived(snapshot.assignments.filter(item => !item.completed).map(item => ({ item, date: validDate(item.dueDate) ? item.dueDate : "" })).sort((a, b) => (a.date || "9999") < (b.date || "9999") ? -1 : (a.date || "9999") > (b.date || "9999") ? 1 : 0));
  const overdue = $derived(active.filter(({ date }) => date && date < today).length);
  const rows = $derived(active.slice(0, family === "small" ? 1 : family === "medium" ? 2 : 4));
</script>

<main class="asg-widget" data-family={family} style:width={`${width}px`} style:height={`${height}px`}>
  <header class="asg-widget-heading">
    <h1>Assignments</h1>
    <span class="asg-widget-count">{active.length} to do</span>
  </header>
  {#if family === "large"}<p class="asg-widget-person">{snapshot.studentName || "Your study pad"}<span>{snapshot.term}</span></p>{/if}
  {#if active.length}
    <div class="asg-widget-rows">
      {#each rows as { item, date } (item.$id)}
        <section class="asg-widget-row">
          <div class="asg-widget-meta"><span class="asg-widget-course">{item.courseCode || "No course"}</span><span class="asg-widget-due" data-overdue={!!date && date < today}>{date ? `${date < today ? "Late · " : date === today ? "Today · " : ""}${shortDate(date)}` : "No due date"}</span></div>
          <p class="asg-widget-title">{item.title || "Untitled assignment"}</p>
        </section>
      {/each}
    </div>
  {:else}
    <div class="asg-widget-empty"><strong>{snapshot.assignments.length ? "All caught up." : "A fresh study pad."}</strong><span>{snapshot.assignments.length ? "A little room to breathe." : "Add your first assignment."}</span></div>
  {/if}
  <footer class="asg-widget-footer"><span>{overdue ? `${overdue} overdue` : active.length > rows.length ? `+ ${active.length - rows.length} more` : "One thing at a time."}</span><span>As of {shortDate(today)}</span></footer>
</main>

<style>
  .asg-widget { box-sizing: border-box; display: flex; flex-direction: column; overflow: hidden; padding: 14px; background: var(--slop-paper); color: var(--slop-ink); font-family: "Avenir Next", sans-serif; }
  .asg-widget-heading { display: flex; flex-shrink: 0; align-items: center; justify-content: space-between; gap: 6px; padding-bottom: 9px; border-bottom: 3px solid var(--slop-plum); }
  h1 { margin: 0; font-size: 18px; line-height: 1; font-weight: 800; letter-spacing: -0.6px; }
  .asg-widget-count { flex-shrink: 0; font-size: 11px; font-weight: 700; color: var(--slop-plum); }
  .asg-widget-person { margin: 10px 0 4px; font-size: 20px; line-height: 1.2; font-weight: 750; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .asg-widget-person span { display: block; margin-top: 3px; font-size: 11px; font-weight: 500; color: var(--slop-muted); }
  .asg-widget-rows { flex: 1; min-height: 0; overflow: hidden; }
  .asg-widget-row { padding: 9px 0; border-bottom: 1px solid var(--slop-rule); }
  .asg-widget-meta { display: flex; gap: 5px; justify-content: space-between; align-items: center; font-size: 10px; line-height: 13px; }
  .asg-widget-course { min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; font-weight: 750; color: var(--slop-plum); }
  .asg-widget-due { flex-shrink: 0; color: var(--slop-muted); }
  .asg-widget-due[data-overdue="true"] { color: var(--slop-accent); }
  .asg-widget-title { margin: 3px 0 0; font-size: 13px; line-height: 17px; font-weight: 600; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .asg-widget-footer { display: flex; flex-shrink: 0; justify-content: space-between; gap: 5px; margin-top: auto; padding-top: 7px; font-size: 9px; line-height: 12px; color: var(--slop-muted); }
  .asg-widget-empty { display: flex; flex: 1; flex-direction: column; justify-content: center; gap: 6px; font-size: 15px; }
  .asg-widget-empty span { font-size: 11px; line-height: 1.4; color: var(--slop-muted); }
  [data-family="small"] .asg-widget-heading { display: block; }
  [data-family="small"] h1 { font-size: 17px; margin-bottom: 4px; }
  [data-family="small"] .asg-widget-row { padding: 8px 0; border-bottom: 0; }
  [data-family="small"] .asg-widget-title { white-space: normal; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
  [data-family="small"] .asg-widget-footer { font-size: 8px; }
  [data-family="medium"] .asg-widget-row { padding: 6px 0; }
  [data-family="large"] { padding: 18px; }
  [data-family="large"] h1 { font-size: 22px; }
  [data-family="large"] .asg-widget-row { padding: 8px 0; }
</style>
