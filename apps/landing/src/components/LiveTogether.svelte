<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";

  type Person = { name: string; hue: number; initial: string };
  type Item = { id: string; text: string; done: boolean; by?: Person };

  const taylor: Person = { name: "Taylor", hue: 28, initial: "T" };
  const jordan: Person = { name: "Jordan", hue: 258, initial: "J" };
  const sam: Person = { name: "Sam", hue: 152, initial: "S" };
  const people = [taylor, jordan, sam];

  const seed: Item[] = [
    { id: "kyoto", text: "Kyoto (cherry blossoms)", done: true, by: taylor },
    { id: "lisbon", text: "Lisbon (food)", done: true, by: jordan },
    { id: "vancouver", text: "Vancouver (hiking)", done: false },
  ];

  let items = $state<Item[]>(seed.map((item) => ({ ...item })));
  let comment = $state({ person: taylor, text: "This is going to be amazing!" });
  let typing = $state<Person | null>(sam);
  let you = $state(false);
  let beat = 0;
  let alive = true;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const live = $derived(you ? 4 : 3);
  const remaining = $derived(items.filter((item) => !item.done).length);

  function toggle(id: string) {
    you = true;
    stop();
    items = items.map((item) =>
      item.id === id ? { ...item, done: !item.done, by: item.done ? undefined : { name: "You", hue: 320, initial: "Y" } } : item,
    );
  }

  const beats: Array<() => void> = [
    () => {
      comment = { person: jordan, text: "Lisbon in May?" };
      typing = taylor;
    },
    () => {
      items = items.map((item) => (item.id === "vancouver" ? { ...item, done: true, by: sam } : item));
      comment = { person: sam, text: "Boots are packed." };
      typing = null;
    },
    () => {
      items = items.map((item) => (item.id === "vancouver" ? { ...item, done: false, by: undefined } : item));
      comment = { person: taylor, text: "This is going to be amazing!" };
      typing = sam;
    },
  ];

  function play() {
    if (!alive || you || prefersReducedMotion.current) return;
    timer = setTimeout(() => {
      if (!alive || you) return;
      beats[beat % beats.length]();
      beat += 1;
      play();
    }, 2800);
  }

  function stop() {
    if (timer) clearTimeout(timer);
    timer = undefined;
  }

  onMount(() => {
    if (!prefersReducedMotion.current) play();
  });
  onDestroy(() => {
    alive = false;
    stop();
  });
</script>

<div class="together">
  <section class="board" aria-label="Interactive collaboration demo with simulated participants">
    <header class="bar">
      <i></i><i></i><i></i>
      <span>Team Notes.tiny</span>
      <div class="live" aria-label={`${live} people live`}>
        <div class="faces" aria-hidden="true">
          {#each people as person}
            <b style={`--hue: ${person.hue}`}>{person.initial}</b>
          {/each}
        </div>
        <em>{live} live</em>
      </div>
    </header>
    <h3>City trip ideas</h3>
    <ul>
      {#each items as item}
        <li>
          <label>
            <input type="checkbox" checked={item.done} onchange={() => toggle(item.id)} />
            <span class:done={item.done}>{item.text}</span>
          </label>
          {#if item.by}
            <small style={`--hue: ${item.by.hue}`}>{item.by.name}</small>
          {/if}
        </li>
      {/each}
      <li class="ghost"><span>Add another…</span></li>
    </ul>
    <footer>
      <span>{remaining ? `${remaining} still open` : "All set."}</span>
      {#if typing}<span class="typing">{typing.name} is typing</span>{/if}
    </footer>
  </section>

  <p class="bubble" style={`--hue: ${comment.person.hue}`} aria-hidden="true">
    <b>{comment.person.initial}</b>
    <span><strong>{comment.person.name}</strong>{comment.text}</span>
  </p>
</div>

<style>
  .together { position: relative; width: min(100%, 520px); margin: 0 auto; padding: 8px 12px 28px 0; }
  .board {
    overflow: hidden;
    border: 1px solid color-mix(in oklch, var(--ink), transparent 88%);
    border-radius: 18px;
    color: var(--ink);
    background: oklch(98.5% .018 92);
    box-shadow: 0 28px 70px oklch(30% .05 250 / .14);
    transform: rotate(-.6deg);
  }
  .bar {
    min-height: 52px;
    padding: 0 14px 0 16px;
    display: flex;
    align-items: center;
    gap: 6px;
    border-bottom: 1px solid oklch(86% .025 80);
  }
  .bar i { width: 9px; height: 9px; border-radius: 50%; background: oklch(70% .18 35); }
  .bar i:nth-child(2) { background: oklch(87% .16 91); }
  .bar i:nth-child(3) { background: oklch(68% .13 145); }
  .bar > span { margin-left: 8px; font-size: .7rem; font-weight: 700; }
  .live { margin-left: auto; display: flex; align-items: center; gap: 8px; }
  .faces { display: flex; }
  .faces b, .bubble b {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border: 2px solid oklch(98.5% .018 92);
    border-radius: 50%;
    color: oklch(99% .01 92);
    background: oklch(52% .16 var(--hue));
    font-size: .58rem;
    font-weight: 750;
  }
  .faces b + b { margin-left: -7px; }
  .live em {
    padding: 3px 8px;
    border-radius: 999px;
    color: oklch(32% .1 145);
    background: oklch(90% .08 145);
    font-size: .58rem;
    font-style: normal;
    font-weight: 750;
  }
  h3 { margin: 18px 20px 8px; font-size: 1.05rem; letter-spacing: -.03em; }
  ul { margin: 0; padding: 4px 0 8px; list-style: none; }
  li {
    min-height: 48px;
    padding: 0 20px;
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid oklch(90% .02 80);
    font-size: .82rem;
  }
  label { display: flex; align-items: center; gap: 10px; cursor: pointer; }
  input { width: 15px; height: 15px; accent-color: oklch(50% .14 145); cursor: pointer; }
  .done { color: var(--muted); text-decoration: line-through; }
  li small {
    padding: 3px 8px;
    border-radius: 999px;
    color: oklch(32% .12 var(--hue));
    background: oklch(94% .05 var(--hue));
    font-size: .58rem;
    font-weight: 700;
  }
  .ghost { color: var(--faint); font-size: .78rem; }
  footer {
    padding: 12px 20px 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--muted);
    font-size: .68rem;
  }
  .typing { color: oklch(42% .08 258); font-weight: 650; }
  .bubble {
    position: absolute;
    right: -8px;
    top: 58%;
    max-width: 220px;
    padding: 8px 10px 8px 8px;
    display: flex;
    align-items: start;
    gap: 8px;
    border: 1px solid color-mix(in oklch, var(--ink), transparent 88%);
    border-radius: 14px 14px 4px 14px;
    background: var(--raised);
    box-shadow: var(--shadow-sm);
    transform: rotate(3deg);
  }
  .bubble span { display: grid; gap: 1px; font-size: .68rem; line-height: 1.35; }
  .bubble strong { color: oklch(38% .12 var(--hue)); font-size: .58rem; }
  @media (max-width: 779px) {
    .board, .bubble { transform: none; }
    .together { padding-right: 0; padding-bottom: 36px; }
    .bubble { right: 8px; top: auto; bottom: -4px; }
  }
  @media (prefers-reduced-motion: reduce) {
    .board, .bubble { transform: none; }
  }
</style>
