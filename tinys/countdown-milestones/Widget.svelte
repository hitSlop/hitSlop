<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps, WidgetProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";

  let { snapshot, family, width, height }: CaptureProps<typeof schema> & WidgetProps = $props();
  const now = new Date();
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  function calendarDay(value: string): number | null {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
    const [year, month, day] = value.split("-").map(Number);
    const date = new Date(0);
    date.setUTCFullYear(year, month - 1, day);
    if (year < 1 || date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day) return null;
    return date.getTime() / 86400000;
  }
  const today = calendarDay(`${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`)!;
  const target = $derived(calendarDay(snapshot.targetDate));
  const days = $derived(target === null ? null : Math.round(target - today));
  const dateLabel = $derived(target === null ? "Choose a date" : `${months[Number(snapshot.targetDate.slice(5, 7)) - 1]} ${Number(snapshot.targetDate.slice(8))}, ${snapshot.targetDate.slice(0, 4)}`);
  const timeLabel = $derived(/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(snapshot.targetTime) ? snapshot.targetTime : "");
  const done = $derived(snapshot.milestones.filter(item => item.done).length);
  const pending = $derived(snapshot.milestones.filter(item => !item.done));
  const rows = $derived(pending.slice(0, family === "large" ? 3 : 2));
</script>

<main class="countdown-widget" data-family={family} style:width={`${width}px`} style:height={`${height}px`}>
  <section class="countdown-widget-page">
    <span class="countdown-widget-binding" aria-hidden="true"></span>
    <h1>{snapshot.title || "A date to remember"}</h1>
    <strong class="countdown-widget-number" data-long={days !== null && String(Math.abs(days)).length > 3}>{days === null ? "—" : days === 0 ? "Today" : Math.abs(days)}</strong>
    <span class="countdown-widget-unit">{days === null ? "Something to look forward to" : days === 0 ? "The day is here" : `${Math.abs(days) === 1 ? "day" : "days"} ${days < 0 ? "since" : "to go"}`}</span>
    <p class="countdown-widget-date">{dateLabel}{#if timeLabel && family !== "small"}<span> · {timeLabel}</span>{/if}</p>
    {#if family === "small"}<span class="countdown-widget-asof">As of {months[now.getMonth()]} {now.getDate()}</span>{/if}
  </section>
  {#if family !== "small"}
    <section class="countdown-widget-steps">
      <header><h2>Along the way</h2><span>{done}/{snapshot.milestones.length}</span></header>
      {#each rows as item (item.$id)}
        <p class="countdown-widget-step"><span class="countdown-widget-box" aria-hidden="true"></span><span>{item.title || "Untitled milestone"}</span></p>
      {:else}
        <p class="countdown-widget-empty">{snapshot.milestones.length ? "Every milestone, done." : "Make a little plan for the big day."}</p>
      {/each}
      <footer><span>{pending.length > rows.length ? `+ ${pending.length - rows.length} more` : "A date worth remembering."}</span><span>As of {months[now.getMonth()]} {now.getDate()}</span></footer>
    </section>
  {/if}
</main>

<style>
  .countdown-widget { box-sizing: border-box; overflow: hidden; display: flex; flex-direction: column; gap: 14px; padding: 15px; background: var(--tiny-paper); color: var(--tiny-ink); font-family: var(--bodyFont); }
  .countdown-widget-page { position: relative; display: flex; flex-shrink: 0; flex-direction: column; align-items: center; min-width: 0; padding: 13px 10px 10px; border: 1px solid var(--tiny-rule); border-radius: 10px; background: var(--tiny-page); box-shadow: 0 3px 0 color-mix(in srgb, var(--tiny-ink) 16%, transparent); }
  .countdown-widget-binding { position: absolute; top: -7px; width: 14px; height: 18px; border-radius: 6px; background: var(--tiny-coral); }
  h1 { width: 100%; margin: 0; font-family: var(--displayFont); font-size: 19px; font-weight: 500; line-height: 21px; text-align: center; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .countdown-widget-number { font-family: var(--displayFont); font-weight: 550; font-size: 62px; line-height: 66px; letter-spacing: -2px; }
  .countdown-widget-number[data-long="true"] { font-size: 43px; }
  .countdown-widget-unit { max-width: 100%; padding: 3px 8px; border-radius: 5px; background: var(--tiny-yellow); font-family: var(--displayFont); font-size: 12px; line-height: 14px; text-align: center; }
  .countdown-widget-date { margin: 8px 0 0; font-size: 11px; line-height: 14px; color: var(--tiny-muted); }
  .countdown-widget-asof { margin-top: 4px; font-size: 8px; color: var(--tiny-muted); }
  .countdown-widget-steps { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; }
  .countdown-widget-steps header { display: flex; justify-content: space-between; align-items: center; gap: 6px; padding-bottom: 5px; font-size: 11px; }
  h2 { margin: 0; font-family: var(--displayFont); font-size: 17px; font-weight: 500; }
  .countdown-widget-step { display: flex; align-items: center; gap: 9px; margin: 0; padding: 8px 0; border-bottom: 1px solid var(--tiny-rule); font-size: 13px; line-height: 17px; }
  .countdown-widget-step > span:last-child { min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .countdown-widget-box { width: 13px; height: 13px; flex-shrink: 0; border: 1px solid var(--tiny-muted); border-radius: 4px; background: var(--tiny-page); }
  .countdown-widget-empty { margin: 8px 0; font-size: 13px; line-height: 1.4; color: var(--tiny-muted); }
  footer { display: flex; gap: 5px; justify-content: space-between; margin-top: auto; padding-top: 7px; font-size: 9px; color: var(--tiny-muted); }
  [data-family="small"] { padding: 12px 9px 9px; }
  [data-family="small"] .countdown-widget-page { flex: 1; padding: 11px 8px 6px; }
  [data-family="small"] h1 { font-size: 15px; line-height: 18px; }
  [data-family="small"] .countdown-widget-number { font-size: 45px; line-height: 47px; }
  [data-family="small"] .countdown-widget-number[data-long="true"] { font-size: 33px; }
  [data-family="small"] .countdown-widget-unit { font-size: 10px; line-height: 12px; padding: 2px 5px; }
  [data-family="small"] .countdown-widget-date { margin-top: 6px; font-size: 10px; }
  [data-family="medium"] { flex-direction: row; padding: 13px 12px 10px; gap: 13px; }
  [data-family="medium"] .countdown-widget-page { width: 143px; padding: 12px 6px 7px; }
  [data-family="medium"] h1 { font-size: 15px; line-height: 18px; }
  [data-family="medium"] .countdown-widget-number { font-size: 45px; line-height: 48px; }
  [data-family="medium"] .countdown-widget-number[data-long="true"] { font-size: 33px; }
  [data-family="medium"] .countdown-widget-date { font-size: 9px; }
  [data-family="medium"] h2 { font-size: 15px; }
  [data-family="medium"] .countdown-widget-step { font-size: 12px; padding: 8px 0; }
  [data-family="medium"] footer { flex-direction: column; gap: 3px; font-size: 8px; }
</style>
