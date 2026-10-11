<script lang="ts">
  import { tick } from "svelte";
  import doc from "./schema";
  import { answerPreview, deckView, formatInterval, nextCard, ratings, reviewCard, type Next } from "./deck";

  interface Props { now: number; offset: number; clock: () => number; onend: () => void }
  const { now, offset, clock, onend }: Props = $props();

  // Session state is view state: which card is up, whether its back is showing, and
  // whether the person chose to study past the budget. Nothing here is saved.
  let pinned = $state<string | null>(null);
  let revealed = $state(false);
  let pending = $state(false);
  let ignoreCap = $state(false);
  let answered = $state(0);
  let next = $state<Next | null>(null);
  let revealButton = $state<HTMLButtonElement>();
  let goodButton = $state<HTMLButtonElement>();

  const card = $derived(pinned ? doc.current.cards.find(row => row.$id === pinned) : undefined);
  const preview = $derived(card ? answerPreview(doc.current, card, now) : null);
  const left = $derived(next?.kind === "card" ? next.left : 0);

  function pick(at = clock()) {
    const view = deckView(doc.current, at, offset);
    next = nextCard(doc.current, view, at, offset, ignoreCap);
    pinned = next.kind === "card" ? next.card.$id : null;
    revealed = false;
    void tick().then(() => revealButton?.focus());
  }
  pick();

  // A card removed elsewhere (another window, an agent) is replaced by the next one; on the
  // done page a learning step that comes due is picked up as the clock moves.
  $effect(() => {
    if (pinned && !card) pick();
  });
  $effect(() => {
    if (next?.kind === "done" && next.nextStepAt !== null && now >= next.nextStepAt) pick(now);
  });

  async function reveal() {
    if (!card || revealed) return;
    revealed = true;
    await tick();
    goodButton?.focus();
  }
  async function answer(rating: number) {
    if (!card || !revealed || pending) return;
    pending = true;
    const id = card.$id;
    try {
      const at = clock();
      await doc.change(tx => reviewCard(tx, doc.current, id, rating, at));
      answered++;
      pick();
    } finally {
      pending = false;
    }
  }
  function studyMore() {
    ignoreCap = true;
    pick();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.metaKey || event.ctrlKey || event.altKey || event.defaultPrevented) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("input, textarea, select, [contenteditable], [role=dialog]")) return;
    if (event.key === "Escape") { event.preventDefault(); onend(); return; }
    // Space and Enter on a focused button are the button's own click.
    const onButton = target?.closest("button");
    if ((event.key === " " || event.key === "Enter") && !onButton && !revealed) { event.preventDefault(); void reveal(); return; }
    if (/^[1-4]$/.test(event.key) && revealed) { event.preventDefault(); void answer(Number(event.key)); }
  }
</script>

<svelte:window {onkeydown} />

<section class="deck-session" aria-label="Study session">
  <div class="deck-session-bar">
    <span class="deck-label">Question {answered + 1}</span>
    <span class="deck-session-left">{#if card}{left} left now{:else}{answered} answered{/if}</span>
    <button class="deck-text-button" onclick={onend}>End <kbd>esc</kbd></button>
  </div>

  {#if card}
    {@const status = card.state === "new" ? "New card" : card.state === "review" ? `Review · seen ${card.reps} ${card.reps === 1 ? "time" : "times"}` : card.state === "learning" ? "Learning" : "Relearning"}
    <article class="deck-question" data-revealed={revealed} aria-live="polite">
      <p class="deck-question-meta">{status}</p>
      <p class="deck-question-front">{card.front || "(blank front)"}</p>
      {#if revealed}
        <div class="deck-answer">
          <span class="deck-answer-label">Answer</span>
          <p>{card.back || "(blank back)"}</p>
        </div>
      {/if}
    </article>

    {#if preview && !preview.ok}
      <div class="deck-unreadable" role="alert">
        <p><strong>This card can't be scheduled.</strong> {preview.reason}</p>
        <button class="deck-text-button" onclick={onend}>Back to the sheet</button>
      </div>
    {:else if !revealed}
      <button class="deck-primary" bind:this={revealButton} onclick={reveal}>Show answer <kbd>space</kbd></button>
    {:else if preview?.ok}
      <div class="deck-bubbles" role="group" aria-label="How well did you remember?">
        {#each ratings as rating (rating.value)}
          {@const interval = formatInterval(preview.options[rating.value].intervalMs)}
          {#if rating.value === 3}
            <button class="deck-bubble" bind:this={goodButton} disabled={pending} onclick={() => answer(rating.value)} aria-label="{rating.label}, next in {interval}. Key {rating.value}.">
              <span class="deck-bubble-mark" aria-hidden="true">{rating.value}</span>
              <span class="deck-bubble-label">{rating.label}</span>
              <span class="deck-bubble-next">{interval}</span>
            </button>
          {:else}
            <button class="deck-bubble" disabled={pending} onclick={() => answer(rating.value)} aria-label="{rating.label}, next in {interval}. Key {rating.value}.">
              <span class="deck-bubble-mark" aria-hidden="true">{rating.value}</span>
              <span class="deck-bubble-label">{rating.label}</span>
              <span class="deck-bubble-next">{interval}</span>
            </button>
          {/if}
        {/each}
      </div>
      <p class="deck-hint">Keys 1–4 · Space picks Good</p>
    {/if}
  {:else if next?.kind === "done"}
    <div class="deck-done">
      {#if next.waiting > 0 && !ignoreCap}
        <p class="deck-done-title">Budget spent for today.</p>
        <p>{next.waiting} {next.waiting === 1 ? "card waits" : "cards wait"} for tomorrow. The forecast already counts them.</p>
        <div class="deck-done-actions">
          <button class="deck-primary" bind:this={revealButton} onclick={onend}>Back to the sheet</button>
          <button class="deck-text-button" onclick={studyMore}>Study {next.waiting} more anyway</button>
        </div>
      {:else if next.nextStepAt !== null}
        <p class="deck-done-title">Nothing due right now.</p>
        <p>The next learning step is due at {new Date(next.nextStepAt).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}. It will appear here when it's due.</p>
        <div class="deck-done-actions"><button class="deck-primary" bind:this={revealButton} onclick={onend}>Back to the sheet</button></div>
      {:else}
        <p class="deck-done-title">Done for today.</p>
        <p>{answered ? `${answered} answered this session. ` : ""}Come back tomorrow for the next cards.</p>
        <div class="deck-done-actions"><button class="deck-primary" bind:this={revealButton} onclick={onend}>Back to the sheet</button></div>
      {/if}
    </div>
  {/if}
</section>
