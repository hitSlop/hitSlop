<script lang="ts">
  // A browser-only preview in Quick Checklist's real colors (examples/slops/quick-checklist/slop.ts).
  type Task = { id: number; text: string; done: boolean };
  let tasks = $state<Task[]>([
    { id: 1, text: "Water the plants", done: true },
    { id: 2, text: "Reply to Sam", done: false },
    { id: 3, text: "Oat milk, obviously", done: false },
  ]);
  let draft = $state("");
  let next = 4;
  let party = $state(0);
  const finished = $derived(tasks.filter((task) => task.done).length);
  const allDone = $derived(tasks.length > 0 && finished === tasks.length);

  function toggle(task: Task): void {
    task.done = !task.done;
    if (tasks.every((t) => t.done)) party += 1;
  }
  function add(event: SubmitEvent): void {
    event.preventDefault();
    const text = draft.trim();
    if (!text) return;
    tasks.push({ id: next++, text, done: false });
    draft = "";
  }
  function remove(task: Task): void {
    tasks = tasks.filter((t) => t !== task);
  }
</script>

<div class="checklist" data-done={allDone}>
  <p class="eyebrow">A little less on your mind.</p>
  <h3>Today</h3>
  <p class="progress" aria-live="polite">{allDone ? "All done. Nicely done." : `${tasks.length - finished} left to do`}<span>{finished} / {tasks.length}</span></p>
  <div class="track" aria-hidden="true"><i style:transform={`scaleX(${tasks.length ? finished / tasks.length : 0})`}></i></div>
  <ul>
    {#each tasks as task (task.id)}
      <li class:done={task.done}>
        <label><input type="checkbox" checked={task.done} onchange={() => toggle(task)} /><span class="box" aria-hidden="true">{task.done ? "✓" : ""}</span><span class="text">{task.text}</span></label>
        <button type="button" aria-label={`Remove ${task.text}`} onclick={() => remove(task)}>×</button>
      </li>
    {/each}
  </ul>
  <form onsubmit={add}>
    <input bind:value={draft} aria-label="New task" placeholder="Add a little thing…" maxlength="40" />
    <button type="submit" aria-label="Add task" disabled={!draft.trim()}>＋</button>
  </form>
  {#key party}
    {#if party > 0 && allDone}
      <div class="confetti" aria-hidden="true">{#each Array(14) as _, i}<i style:--i={i}></i>{/each}</div>
    {/if}
  {/key}
</div>

<style>
  .checklist { --surface: #e98996; --paper: #fff9f3; --ink: #432830; --muted: #82636a; --accent: #a43d59; --rule: #ead9cb; position: relative; padding: 18px 18px 16px; color: var(--ink); background: var(--paper); font-family: "Onest", "Avenir Next", sans-serif; }
  .eyebrow { margin: 0; color: var(--muted); font-size: 10px; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; }
  h3 { margin: 2px 0 8px; font-family: Georgia, "Times New Roman", serif; font-size: 30px; line-height: 1; letter-spacing: -.02em; }
  .progress { margin: 0 0 6px; display: flex; justify-content: space-between; color: var(--muted); font-size: 12px; font-weight: 700; }
  .track { height: 6px; overflow: hidden; border-radius: 99px; background: var(--rule); }
  .track i { display: block; height: 100%; border-radius: inherit; background: var(--accent); transform-origin: left; transition: transform 420ms cubic-bezier(.34, 1.56, .64, 1); }
  ul { margin: 12px 0 8px; padding: 0; list-style: none; }
  li { display: flex; align-items: center; gap: 6px; border-bottom: 1px solid var(--rule); }
  label { flex: 1; min-height: 42px; display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: 14px; font-weight: 600; }
  input[type="checkbox"] { position: absolute; opacity: 0; pointer-events: none; }
  .box { width: 22px; height: 22px; flex: none; display: grid; place-items: center; border: 2px solid var(--accent); border-radius: 7px; color: var(--paper); font-size: 13px; font-weight: 900; transition: background-color 180ms, transform 260ms cubic-bezier(.34, 1.56, .64, 1); }
  li.done .box { background: var(--accent); transform: rotate(-8deg) scale(1.05); }
  li.done .text { color: var(--muted); text-decoration: line-through; text-decoration-thickness: 2px; }
  label:has(input:focus-visible) .box { outline: 3px solid var(--surface); outline-offset: 2px; }
  li button { width: 26px; height: 26px; padding: 0; border: 0; border-radius: 50%; color: var(--muted); background: transparent; font-size: 17px; cursor: pointer; opacity: .55; }
  li button:hover, li button:focus-visible { opacity: 1; background: #f6e2dc; }
  form { display: flex; gap: 8px; }
  form input { flex: 1; min-width: 0; height: 38px; padding: 0 12px; border: 1px solid var(--rule); border-radius: 12px; color: var(--ink); background: #fff; font: inherit; font-size: 13px; }
  form button { width: 38px; height: 38px; padding: 0; border: 0; border-radius: 50%; color: var(--paper); background: var(--accent); font-size: 18px; font-weight: 800; cursor: pointer; transition: transform 200ms cubic-bezier(.34, 1.56, .64, 1); }
  form button:disabled { opacity: .45; cursor: default; }
  form button:not(:disabled):hover { transform: rotate(90deg); }
  .confetti { position: absolute; inset: 0; pointer-events: none; overflow: hidden; }
  .confetti i { position: absolute; left: 50%; top: 40%; width: 8px; height: 12px; border-radius: 2px; background: hsl(calc(var(--i) * 47) 85% 62%); animation: burst 1100ms cubic-bezier(.2, .8, .3, 1) forwards; --a: calc(var(--i) * 25.7deg); }
  @keyframes burst { to { transform: rotate(var(--a)) translateY(-120px) rotate(calc(var(--a) * 3)); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .confetti { display: none; } }
</style>
