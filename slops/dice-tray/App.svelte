<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { EditableText, notify } from "hitslop/svelte";
  import { onMount } from "svelte";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import doc, { kinds, maxModifier, type Kind } from "./schema";
  import {
    addDie, clearHistory, fairRandom, removeDie, roll, rollAgain, setModifier, setPool, trayView, type RollView,
  } from "./dice";
  import DieGlyph from "./DieGlyph.svelte";
  import History from "./History.svelte";
  import Readout from "./Readout.svelte";
  import Tray from "./Tray.svelte";

  const view = $derived(trayView(doc.current));
  const shown = $derived(view.last ? { id: view.last.$id, seed: view.last.seed, dice: view.last.dice } : undefined);
  // View state: which roll is mid-tumble, motion preference and the clock for "8:07 PM".
  let rollingId = $state<string>();
  let reduced = $state(false);
  let now = $state(Date.now());
  const rolling = $derived(!!rollingId && rollingId === view.lastView?.id);
  const modifier = $derived(doc.current.modifier);
  let announcement = $state("");

  function settle(id: string) {
    if (rollingId === id) rollingId = undefined;
    const last = view.lastView;
    if (last && last.id === id) announcement = `Rolled ${last.total}: ${last.breakdown}${last.natural ? `, natural ${last.natural}` : ""}.`;
  }

  async function rollPool() {
    if (!view.dice) return;
    await doc.change((tx) => roll(tx, doc.current, fairRandom, Date.now()));
  }
  async function again(earlier: RollView) {
    await doc.change((tx) => rollAgain(tx, doc.current, earlier.id, fairRandom, Date.now()));
  }
  function add(kind: Kind) {
    if (view.full) { notify("The tray holds 12 dice at most."); return; }
    void doc.change((tx) => addDie(tx, doc.current, kind));
  }
  function remove(kind: Kind) {
    void doc.change((tx) => removeDie(tx, doc.current, kind));
  }
  function nudge(by: number) {
    const next = Math.max(-maxModifier, Math.min(maxModifier, modifier + by));
    if (next !== modifier) void doc.change((tx) => setModifier(tx, next));
  }
  function emptyPool() {
    void doc.change((tx) => setPool(tx, { d4: 0, d6: 0, d8: 0, d10: 0, d12: 0, d20: 0 }, 0));
  }
  async function clearAll() {
    const count = await doc.change((tx) => clearHistory(tx, doc.current));
    notify(`${count} ${count === 1 ? "roll" : "rolls"} cleared. Undo brings them back.`);
  }

  onMount(() => {
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const onMotion = () => (reduced = motion.matches);
    onMotion();
    motion.addEventListener("change", onMotion);
    const clock = setInterval(() => (now = Date.now()), 30_000);
    // R rolls from anywhere except while typing.
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "r" || event.metaKey || event.ctrlKey || event.altKey || event.repeat) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable]")) return;
      event.preventDefault();
      void rollPool();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      motion.removeEventListener("change", onMotion);
      clearInterval(clock);
      window.removeEventListener("keydown", onKey);
    };
  });

  const trayLabel = $derived(
    !view.lastView ? "An empty dice tray."
      : rolling ? "The dice are tumbling."
      : `Dice tray showing ${view.lastView.values.map((die) => `${die.kind} ${die.value}`).join(", ")}.`,
  );
</script>

<main class="tray-shell">
  <Readout roll={view.lastView} {rolling}>
    {#snippet title()}
      <EditableText class="plate-title" field={doc.fields.title} label="Tray name" placeholder="Name this table" />
    {/snippet}
  </Readout>
  <p class="sr-only" aria-live="polite">{announcement}</p>

  <section class="well" aria-label="Dice tray">
    <Tray roll={shown} {reduced} label={trayLabel} onRolling={(id) => (rollingId = id)} onSettled={settle} />
  </section>

  <div class="rack" role="group" aria-label="Dice pool">
    {#each kinds as kind (kind)}
      {@const count = view.pool[kind]}
      <div class="rack-slot" data-active={count > 0}>
        <button
          class="rack-die"
          aria-label={`Add a ${kind}${count ? `, ${count} in the pool` : ""}`}
          disabled={view.full}
          onclick={() => add(kind)}
          oncontextmenu={(event) => { event.preventDefault(); remove(kind); }}
        >
          <DieGlyph {kind} size={34} />
          <span class="rack-count">{count ? `${count}×${kind}` : kind}</span>
        </button>
        {#if count}
          <button class="rack-minus" aria-label={`Remove a ${kind}`} onclick={() => remove(kind)}><Minus size={14} aria-hidden="true" /></button>
        {/if}
      </div>
    {/each}
  </div>

  <div class="controls">
    <div class="modifier" role="group" aria-label="Modifier">
      <button aria-label="Lower the modifier" disabled={modifier <= -maxModifier} onclick={() => nudge(-1)}><Minus size={18} aria-hidden="true" /></button>
      <output aria-live="polite" aria-label={`Modifier ${modifier}`}>{modifier < 0 ? "−" : "+"}{Math.abs(modifier)}</output>
      <button aria-label="Raise the modifier" disabled={modifier >= maxModifier} onclick={() => nudge(1)}><Plus size={18} aria-hidden="true" /></button>
    </div>
    <button class="roll" disabled={!view.dice} onclick={rollPool} aria-keyshortcuts="R">
      <span class="roll-verb">Roll</span>
      <span class="roll-formula">{view.dice ? view.next : "Add a die first"}</span>
    </button>
    <button class="empty" aria-label="Empty the pool" title="Empty the pool" disabled={!view.dice && !modifier} onclick={emptyPool}><X size={18} aria-hidden="true" /></button>
  </div>

  <section class="slip" aria-label="Recent rolls">
    <div class="slip-head">
      <h2>Recent rolls</h2>
      <button class="slip-clear" disabled={!view.history.length} onclick={clearAll}><Trash size={14} aria-hidden="true" /> Clear</button>
    </div>
    <div class="slip-scroll">
      <History rolls={view.history} {now} onAgain={again} />
    </div>
  </section>
</main>
