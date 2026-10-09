<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  type DemoTask = { $id: string; text: string; done: boolean; archived: boolean };
  const initialTasks = (): DemoTask[] => [
    { $id: "draft", text: "Draft proposal", done: true, archived: false },
    { $id: "invoice", text: "Send invoice", done: false, archived: false },
  ];
  let tasks = $state(initialTasks());
  const command = `slop call Today.slop addTask --args '{"text":"Book train"}'`;
  const editCommand = `slop apply Today.slop --op '{"type":"set","path":["tasks",{"id":"train"},"done"],"value":true}'`;
  let direction = $state<"idle" | "ui" | "agent">("idle");
  let settleTimer: ReturnType<typeof setTimeout> | undefined;
  const completed = $derived(tasks.filter(task => task.done).length);
  const train = $derived(tasks.find(task => task.$id === "train"));
  const snapshot = $derived(JSON.stringify({ title: "Today", tasks }, null, 2));
  const describeResponse = `Fields
  ["tasks"]: list (insert, remove, move, replace)
  ["tasks",{"id":"$id"},"done"]: boolean (set)

Commands (slop call PATH NAME --args JSON)
  addTask — Add an unfinished task and return its stable row ID.
    args: {"additionalProperties":false,"properties":{"text":{"type":"string"}},"required":["text"],"type":"object"}`;
  const callResponse = JSON.stringify({ result: { id: "train" }, ids: ["train"] });
  const editResponse = JSON.stringify({ ids: [] });
  let applied = $state(false);
  let root: HTMLElement;
  let typed = $state(command);
  let timers: ReturnType<typeof setTimeout>[] = [];
  let interacted = false;

  function stopAnimation() {
    interacted = true;
    timers.forEach(clearTimeout);
    typed = command;
  }
  function showDirection(next: "ui" | "agent") {
    direction = next;
    clearTimeout(settleTimer);
    settleTimer = setTimeout(() => { direction = "idle"; }, 1600);
  }
  function updateFromUI(id: string, done: boolean) {
    stopAnimation();
    tasks = tasks.map(task => task.$id === id ? { ...task, done } : task);
    showDirection("ui");
  }
  function addTask() {
    stopAnimation();
    if (train) return;
    tasks = [...tasks, { $id: "train", text: "Book train", done: false, archived: false }];
    showDirection("agent");
  }
  function apply() {
    stopAnimation();
    tasks = tasks.map(task => task.$id === "train" ? { ...task, done: true } : task);
    applied = true;
    showDirection("agent");
  }
  function reset() {
    stopAnimation();
    clearTimeout(settleTimer);
    tasks = initialTasks();
    applied = false;
    direction = "idle";
  }
  // Demonstrate the named action once; visitors can then try the direct edit.
  onMount(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry?.isIntersecting) return;
      observer.disconnect();
      if (interacted) return;
      typed = "";
      let delay = 500;
      for (let i = 1; i <= command.length; i++) {
        delay += 14;
        timers.push(setTimeout(() => { typed = command.slice(0, i); }, delay));
      }
      timers.push(setTimeout(addTask, delay + 700));
    }, { threshold: .4 });
    observer.observe(root);
    return () => observer.disconnect();
  });
  onDestroy(() => { clearTimeout(settleTimer); timers.forEach(clearTimeout); });
</script>

<div class="sync-demo" data-direction={direction} bind:this={root}>
  <section class="sync-window" aria-label="Editable checklist interface">
    <div class="window-bar"><i></i><i></i><i></i><span>Today.slop</span></div>
    <div class="checklist">
      {#each tasks as task (task.$id)}
        <label><input type="checkbox" checked={task.done} onchange={event => updateFromUI(task.$id, event.currentTarget.checked)} /><span>{task.text}</span></label>
      {/each}
    </div>
    <p><strong>{completed}</strong> of {tasks.length} done <small aria-live="polite">{direction === "agent" ? "Operation applied" : direction === "ui" ? "State updated" : "Try a checkbox"}</small></p>
  </section>
  <div class="sync-arrows" aria-hidden="true"><span class:active={direction === "ui"}>App → state</span><b>⇄</b><span class:active={direction === "agent"}>CLI → app</span></div>
  <section class="sync-code" aria-label="Simulated document CLI">
    <header><span class="status-dot"></span><span>Document commands</span><small>Illustration</small></header>
    <ol class="command-example">
      <li>
        <p>Discover the app’s actions</p>
        <pre><code>slop describe Today.slop</code></pre>
        <details><summary>Response · excerpt</summary><pre><code>{describeResponse}</code></pre></details>
      </li>
      <li>
        <p>Run a named action</p>
        <pre><code>{typed}{#if typed !== command}<span class="caret" aria-hidden="true">▌</span>{/if}</code></pre>
        <button type="button" onclick={addTask} disabled={!!train}>Add “Book train”</button>
        <p class="response-label">{train ? "Response" : "Example response"}</p>
        <pre class="response"><code>{callResponse}</code></pre>
      </li>
      <li>
        <p>Read its ID, then edit a field</p>
        <pre><code>slop get Today.slop</code></pre>
        <details><summary>Response · current document</summary><pre><code>{snapshot}</code></pre></details>
        <pre><code>{editCommand}</code></pre>
        <small>Use the task’s $id from get; this illustration uses “train”.</small>
        <button type="button" onclick={apply} disabled={!train || train.done}>Mark it done</button>
        <p class="response-label">{applied ? "Response" : "Example response"}</p>
        <pre class="response"><code>{editResponse}</code></pre>
      </li>
    </ol>
    <div class="demo-footer"><small>Illustration only. Nothing runs on your Mac.</small><button type="button" onclick={reset}>Reset demo</button></div>
  </section>
</div>

<style>
  .sync-demo { min-width: 0; width: 100%; display: grid; gap: 22px; align-items: center; }
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
    .sync-demo { grid-template-columns: minmax(0, .8fr) auto minmax(0, 1.4fr); gap: 22px; }
    .sync-arrows { flex-direction: column; }
  }
  @media (prefers-reduced-motion: reduce) { .sync-arrows b, .sync-arrows span { transition: none; } }
  .command-example { margin: 0; padding: 18px 18px 0 40px; color: oklch(88% .025 320); }
  .command-example p { margin: 0 0 8px; font-size: .88rem; }
  .command-example li { padding-left: 3px; margin-bottom: 22px; }
  .command-example li::marker { color: oklch(84% .1 145); font-weight: 800; }
  .command-example pre { white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.7; color: oklch(83% .045 96); font-size: .85rem; margin: 8px 0; }
  .command-example button { margin: 8px 0 10px; padding: 10px 14px; border: 0; border-radius: 5px; background: oklch(84% .1 145); color: oklch(22% .055 320); font: inherit; font-size: .9rem; font-weight: 800; cursor: pointer; transition: transform 200ms var(--spring); }
  .command-example button:hover:enabled { transform: translateY(-2px) rotate(-1deg); }
  .command-example button:disabled { opacity: .5; cursor: default; }
  .demo-footer { padding: 0 18px; display: flex; flex-wrap: wrap; gap: 10px; align-items: center; color: oklch(80% .03 320); }
  .demo-footer button { border: 0; background: none; color: oklch(84% .1 145); padding: 6px 0; font: inherit; font-size: .8rem; text-decoration: underline; text-underline-offset: 3px; cursor: pointer; }
  .caret { animation: blink 800ms steps(1) infinite; }
  @media (prefers-reduced-motion: reduce) { .caret { animation: none; } .command-example button { transition: none; } }
  @keyframes blink { 50% { opacity: 0; } }
  .command-example small { display: block; font-size: .8rem; color: oklch(80% .03 320); }
  .command-example .response-label { margin: 8px 0 0; font-size: .75rem; color: oklch(80% .03 320); }
  .command-example .response { color: oklch(84% .1 145); }
  .command-example details { padding: 8px 0; }
  .demo-footer { padding-bottom: 18px; }
  button:focus-visible, summary:focus-visible { outline: 2px solid oklch(84% .1 145); outline-offset: 3px; }
  details { padding: 16px 18px; color: oklch(88% .025 320); font-size: .85rem; }
  summary { cursor: pointer; }
  pre { overflow: auto; font-size: .8rem; }
</style>
