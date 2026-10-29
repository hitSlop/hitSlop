<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import type { CaptureProps, WidgetProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { intakeView } from "./intake";

  let { snapshot, family, width, height }: CaptureProps<typeof schema> & WidgetProps = $props();
  const view = $derived(intakeView(snapshot));
  const level = $derived(Math.max(0, Math.min(100, view.percent)));
  const remaining = $derived(Math.max(0, snapshot.target - view.total));
  const drinks = $derived([...snapshot.logs].sort((a, b) => a.time < b.time ? 1 : a.time > b.time ? -1 : 0).slice(0, 3));
  const amount = (value: number) => String(Math.round(value * 100) / 100);
</script>

<main class="water-widget" data-family={family} style:width={`${width}px`} style:height={`${height}px`} aria-label="Water tracker widget">
  <header><span>Hydration 01</span><span class="water-percent">{view.percent}%</span></header>
  <div class="water-body">
    <section class="water-tank" aria-label={`${view.percent}% of daily goal`}>
      <div class="water-fill" style:height={`${level}%`} aria-hidden="true"></div>
      <div class="water-readout">
        <strong>{amount(view.total)}</strong>
        <span class="water-unit">{snapshot.unit}</span>
      </div>
    </section>
    <div class="water-summary">
      <p class="water-goal">of {amount(snapshot.target)} {snapshot.unit}</p>
      {#if family !== "small"}
        <h1>{view.complete ? "Goal reached." : "A little more water."}</h1>
        <p class="water-remaining">{view.complete ? "Every sip adds up." : `${amount(remaining)} ${snapshot.unit} to go`}</p>
      {/if}
      {#if family === "medium"}<p class="water-count">{view.count} {view.count === 1 ? "drink" : "drinks"} logged</p>{/if}
    </div>
  </div>
  {#if family === "large"}
    <section class="water-log" aria-label="Latest drinks">
      <div class="water-log-heading"><span>Latest drinks</span><span>{view.count} logged</span></div>
      {#each drinks as drink (drink.$id)}
        <div class="water-drink"><time>{drink.time}</time><span>{drink.label || "Drink"}</span><strong>{amount(drink.amount)} {snapshot.unit}</strong></div>
      {:else}
        <p class="water-empty">Your first glass is a good start.</p>
      {/each}
    </section>
  {/if}
</main>

<style>
  .water-widget { box-sizing: border-box; display: flex; flex-direction: column; gap: 10px; padding: 14px; overflow: hidden; background: var(--tiny-surface); color: var(--tiny-ink); font-family: var(--font); }
  header { display: flex; justify-content: space-between; align-items: center; gap: 8px; flex-shrink: 0; color: var(--tiny-accent); font-size: 10px; font-weight: 800; letter-spacing: 0.08em; text-transform: uppercase; }
  .water-percent { font-family: var(--mono); letter-spacing: 0; }
  .water-body { display: flex; flex: 1; min-height: 0; gap: 16px; }
  .water-tank { position: relative; display: flex; flex-direction: column; justify-content: center; overflow: hidden; background: var(--tiny-chamber); border: 1px solid var(--tiny-border); border-radius: 14px; width: 124px; flex-shrink: 0; }
  .water-fill { position: absolute; bottom: 0; left: 0; width: 100%; background: linear-gradient(180deg, var(--tiny-accentDeep), var(--tiny-panel)); border-top: 2px solid var(--tiny-accent); }
  .water-readout { position: relative; display: flex; flex-direction: column; align-items: center; gap: 3px; padding: 8px; overflow: hidden; }
  .water-readout strong { max-width: 100%; overflow: hidden; font-family: var(--mono); font-size: 32px; font-weight: 800; line-height: 1.1; letter-spacing: -0.04em; color: var(--tiny-ink); }
  .water-unit { max-width: 100%; overflow: hidden; white-space: nowrap; font-size: 11px; font-weight: 700; }
  .water-summary { display: flex; flex-direction: column; justify-content: center; gap: 8px; min-width: 0; flex: 1; }
  .water-goal { margin: 0; color: var(--tiny-muted); font-size: 11px; line-height: 1.2; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  h1 { margin: 0; font-size: 22px; font-weight: 700; line-height: 1.1; letter-spacing: -0.03em; }
  .water-remaining { margin: 0; font-size: 12px; color: var(--tiny-accent); }
  .water-count { margin: 2px 0 0; color: var(--tiny-muted); font-size: 10px; }
  .water-log { display: flex; flex-direction: column; flex-shrink: 0; }
  .water-log-heading { display: flex; justify-content: space-between; margin-bottom: 5px; color: var(--tiny-muted); font-size: 10px; }
  .water-drink { display: flex; align-items: center; gap: 9px; height: 31px; border-top: 1px solid var(--tiny-border); font-size: 11px; }
  .water-drink time { color: var(--tiny-muted); font-family: var(--mono); font-size: 10px; flex-shrink: 0; }
  .water-drink span { min-width: 0; flex: 1; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .water-drink strong { flex-shrink: 0; max-width: 100px; overflow: hidden; white-space: nowrap; font-weight: 600; color: var(--tiny-accent); }
  .water-empty { margin: 5px 0; color: var(--tiny-muted); font-size: 12px; }
  [data-family="small"] { padding: 12px; gap: 8px; }
  [data-family="small"] .water-body { flex-direction: column; gap: 7px; }
  [data-family="small"] .water-tank { width: 100%; flex: 1; }
  [data-family="small"] .water-summary { flex: 0; text-align: center; }
  [data-family="small"] .water-readout strong { font-size: 29px; }
  [data-family="medium"] .water-tank { width: 118px; }
  [data-family="medium"] h1 { font-size: 19px; }
  [data-family="medium"] .water-summary { gap: 6px; }
  [data-family="large"] .water-tank { width: 144px; }
  [data-family="large"] .water-readout strong { font-size: 36px; }
</style>
