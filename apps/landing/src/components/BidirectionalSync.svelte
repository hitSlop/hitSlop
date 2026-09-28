<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  type DemoItem = { $id: string; title: string; done: boolean };

  let items = $state<DemoItem[]>([
    { $id: "draft", title: "Draft proposal", done: true },
    { $id: "invoice", title: "Send invoice", done: true },
    { $id: "train", title: "Book train", done: false },
  ]);
  const command = `slop apply Today.slop --op '{"type":"set","path":["items",{"id":"train"},"done"],"value":true}'`;
  let direction = $state<"idle" | "ui" | "agent">("idle");
  let settleTimer: ReturnType<typeof setTimeout> | undefined;
  const completed = $derived(items.filter(item => item.done).length);
  const snapshot = $derived(JSON.stringify({ items }, null, 2));

  function showDirection(next: "ui" | "agent") {
    direction = next;
    clearTimeout(settleTimer);
    settleTimer = setTimeout(() => { direction = "idle"; }, 1600);
  }
  function updateFromUI(id: string, done: boolean) {
    items = items.map(item => item.$id === id ? { ...item, done } : item);
    showDirection("ui");
  }
  function apply() {
    items = items.map(item => item.$id === "train" ? { ...item, done: true } : item);
    showDirection("agent");
  }
  // On first view the "agent" types the command and presses the button once, so the
  // round trip is visible without reading. Visitors can then try both directions.
  let root: HTMLElement;
  let typed = $state(command);
  let pressing = $state(false);
  let timers: ReturnType<typeof setTimeout>[] = [];
  onMount(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry?.isIntersecting) return;
      observer.disconnect();
      if (items.find(item => item.$id === "train")?.done) return;
      typed = "";
      let delay = 500;
      for (let i = 1; i <= command.length; i++) {
        delay += 14;
        timers.push(setTimeout(() => { typed = command.slice(0, i); }, delay));
      }
      timers.push(setTimeout(() => { pressing = true; }, delay + 350));
      timers.push(setTimeout(() => { pressing = false; apply(); }, delay + 700));
    }, { threshold: .55 });
    observer.observe(root);
    return () => observer.disconnect();
  });
  onDestroy(() => { clearTimeout(settleTimer); timers.forEach(clearTimeout); });
</script>

<div class="sync-demo" data-direction={direction} bind:this={root}>
  <section class="sync-window" aria-label="Editable checklist interface">
    <div class="window-bar"><i></i><i></i><i></i><span>Today.slop</span></div>
    <div class="checklist">
      {#each items as item (item.$id)}
        <label><input type="checkbox" checked={item.done} onchange={event => updateFromUI(item.$id, event.currentTarget.checked)} /><span>{item.title}</span></label>
      {/each}
    </div>
    <p><strong>{completed}</strong> of {items.length} done <small aria-live="polite">{direction === "agent" ? "Operation applied" : direction === "ui" ? "State updated" : "Try a checkbox"}</small></p>
  </section>
  <div class="sync-arrows" aria-hidden="true"><span class:active={direction === "ui"}>App → state</span><b>⇄</b><span class:active={direction === "agent"}>CLI → app</span></div>
  <section class="sync-code" aria-label="Simulated document CLI">
    <header><span class="status-dot"></span><span>Document commands</span><small>Illustration</small></header>
    <div class="command-example">
      <p>Read the document</p>
      <code>slop get Today.slop</code>
      <p>Mark “Book train” done</p>
      <pre><code>{typed}{#if typed !== command}<span class="caret" aria-hidden="true">▌</span>{/if}</code></pre>
      <button type="button" onclick={apply} class:pressing>Run it again</button>
      <small>This example runs only on this page.</small>
    </div>
    <details><summary>Current state · slop get</summary><pre>{snapshot}</pre></details>
  </section>
</div>

<style>
  .sync-demo { width: 100%; display: grid; gap: 22px; align-items: center; }
  .sync-window { overflow: hidden; border: 1px solid oklch(80% .025 320 / .24); border-radius: 17px; color: oklch(20% .018 255); background: oklch(97% .018 85); box-shadow: 0 38px 90px oklch(8% .04 320 / .3); transform: rotate(-1deg); }
  .window-bar { min-height: 49px; padding: 0 15px; display: flex; align-items: center; gap: 6px; border-bottom: 1px solid oklch(82% .025 80); }
  .window-bar i { width: 9px; height: 9px; border-radius: 50%; background: oklch(70% .18 35); }
  .window-bar i:nth-child(2) { background: oklch(87% .16 91); }
  .window-bar i:nth-child(3) { background: oklch(68% .13 145); }
  .window-bar span { margin-left: 8px; font-size: .85rem; font-weight: 700; }
  .checklist label { min-height: 56px; padding: 0 19px; display: flex; align-items: center; gap: 11px; border-bottom: 1px solid oklch(86% .025 80); font-size: .95rem; cursor: pointer; }
  .checklist input { width: 16px; height: 16px; accent-color: oklch(50% .14 145); cursor: pointer; }
  .sync-window > p { margin: 0; padding: 14px 19px; display: flex; align-items: center; gap: 4px; color: oklch(46% .02 255); font-size: .85rem; }
  .sync-window > p strong { color: oklch(20% .018 255); }
  .sync-window > p small { margin-left: auto; color: oklch(43% .09 145); font-size: .8rem; font-weight: 700; }
  .sync-arrows { display: flex; align-items: center; justify-content: center; gap: 13px; color: oklch(72% .04 320); }
  .sync-arrows b { width: 44px; height: 44px; display: grid; place-items: center; border: 1px solid oklch(80% .035 320 / .22); border-radius: 50%; color: oklch(22% .055 320); background: oklch(84% .1 145); font-size: 1.3rem; transition: transform 240ms cubic-bezier(.22,1,.36,1), background-color 240ms cubic-bezier(.22,1,.36,1); }
  .sync-arrows span { font-size: .78rem; font-weight: 700; letter-spacing: .07em; text-transform: uppercase; transition: color 180ms cubic-bezier(.22,1,.36,1); }
  .sync-arrows span.active { color: oklch(87% .12 145); }
  .sync-demo[data-direction="ui"] .sync-arrows b { transform: rotate(16deg) scale(1.08); }
  .sync-demo[data-direction="agent"] .sync-arrows b { transform: rotate(-16deg) scale(1.08); }
  .sync-code { min-width: 0; position: relative; overflow: hidden; border: 1px solid oklch(80% .025 320 / .2); border-radius: 15px; background: oklch(16% .04 320); box-shadow: 0 22px 55px oklch(7% .03 320 / .28); transform: rotate(.8deg); }
  .sync-code header { min-height: 48px; padding: 0 16px; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 9px; border-bottom: 1px solid oklch(82% .03 320 / .12); }
  .status-dot { width: 7px; height: 7px; border-radius: 50%; background: oklch(72% .13 145); }
  .sync-code header small { color: oklch(68% .04 320); font-size: .78rem; }
  @media (min-width: 620px) {
    .sync-demo { grid-template-columns: 1fr auto 1.08fr; gap: 14px; }
    .sync-arrows { flex-direction: column; }
  }
  @media (prefers-reduced-motion: reduce) { .sync-arrows b, .sync-arrows span { transition: none; } }
  .command-example { padding: 18px; color: oklch(88% .025 320); }
  .command-example p { margin: 0 0 8px; font-size: .88rem; }
  .command-example > code { display: block; margin-bottom: 24px; font-size: .85rem; }
  .command-example pre { white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.7; color: oklch(83% .045 96); font-size: .85rem; min-height: 5.1em; }
  .command-example button { margin: 8px 0 10px; padding: 10px 14px; border: 0; border-radius: 5px; background: oklch(84% .1 145); color: oklch(22% .055 320); font: inherit; font-size: .9rem; font-weight: 800; cursor: pointer; transition: transform 200ms var(--spring); }
  .command-example button:hover { transform: translateY(-2px) rotate(-1deg); }
  .command-example button.pressing { transform: translateY(2px) scale(.94); box-shadow: 0 0 0 6px oklch(84% .1 145 / .35); }
  .caret { animation: blink 800ms steps(1) infinite; }
  @keyframes blink { 50% { opacity: 0; } }
  .command-example small { display: block; font-size: .8rem; color: oklch(80% .03 320); }
  button:focus-visible, summary:focus-visible { outline: 2px solid oklch(84% .1 145); outline-offset: 3px; }
  details { padding: 16px 18px; color: oklch(88% .025 320); font-size: .85rem; }
  summary { cursor: pointer; }
  pre { overflow: auto; font-size: .8rem; }
</style>
