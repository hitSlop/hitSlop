<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { Tween, prefersReducedMotion } from "svelte/motion";
  import { cubicOut } from "svelte/easing";
  import { Button, Dialog, Select, Tabs } from "bits-ui";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Layers from "@lucide/svelte/icons/layers";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import Shuffle from "@lucide/svelte/icons/shuffle";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { capture } from "@hitslop/document/capture";
  import schema from "./schema";

  type LeitnerBox = 1 | 2 | 3 | 4;

  const FLIP_MS = 420;
  const BOXES: Array<{ id: LeitnerBox; label: string }> = [
    { id: 1, label: "New" },
    { id: 2, label: "Review" },
    { id: 3, label: "Known" },
    { id: 4, label: "Mastered" },
  ];

  const doc = useDocument(schema);
  let addingDeck = $state(false);
  let editing = $state(false);
  let editingExisting = $state(false);
  let draftFront = $state("");
  let draftBack = $state("");
  let draftDeck = $state("");
  let flipped = $state(false);
  let initialized = false;
  let lastCardId: string | null = null;

  const activeDeck = $derived(doc.current.decks.find((deck) => deck.deckKey === doc.current.selectedDeckId) ?? doc.current.decks[0] ?? null);
  const visibleCards = $derived.by(() => {
    const cards = activeDeck?.cards ?? [];
    if (doc.current.selectedBox === 0) return cards;
    return cards.filter((card) => card.box === doc.current.selectedBox);
  });
  const cardIndex = $derived(visibleCards.length === 0 ? 0 : Math.min(Math.max(0, doc.current.cardIndex), visibleCards.length - 1));
  const currentCard = $derived(visibleCards[cardIndex] ?? null);
  const boxCounts = $derived.by(() => {
    const counts: Record<LeitnerBox, number> = { 1: 0, 2: 0, 3: 0, 4: 0 };
    for (const card of activeDeck?.cards ?? []) counts[clampBox(card.box)] += 1;
    return counts;
  });
  const boxLabel = $derived(currentCard ? (BOXES[clampBox(currentCard.box) - 1]?.label ?? "New") : "New");
  const rotationTarget = $derived(flipped ? 180 : 0);
  const rotation = new Tween(untrack(() => rotationTarget), { duration: FLIP_MS, easing: cubicOut });
  const deckItems = $derived(doc.current.decks.map((deck) => ({ value: deck.deckKey, label: deck.name })));
  const dialogOpen = $derived(addingDeck || editing);
  const exportStages = $derived([
    { id: 0, label: "All", count: activeDeck?.cards.length ?? 0 },
    { id: 1, label: "New", count: boxCounts[1] },
    { id: 2, label: "Review", count: boxCounts[2] },
    { id: 3, label: "Known", count: boxCounts[3] },
    { id: 4, label: "Mastered", count: boxCounts[4] },
  ]);

  function clampBox(value: number): LeitnerBox {
    if (value <= 1) return 1;
    if (value >= 4) return 4;
    return value as LeitnerBox;
  }

  function selectDeck(deckKey: string): void {
    doc.change((tx) => {
      tx.fields.selectedDeckId.set(deckKey);
      tx.fields.selectedBox.set(0);
      tx.fields.cardIndex.set(0);
    });
    flipped = false;
    editing = false;
  }

  function selectBox(box: number): void {
    doc.change((tx) => {
      tx.fields.selectedBox.set(box);
      tx.fields.cardIndex.set(0);
    });
    flipped = false;
  }

  function flipCard(): void {
    if (!currentCard || dialogOpen) return;
    flipped = !flipped;
  }

  function grade(good: boolean): void {
    const deck = activeDeck;
    const graded = currentCard;
    if (!deck || !graded || dialogOpen || !flipped) return;
    const previousIndex = cardIndex;
    const nextId = visibleCards[(cardIndex + 1) % visibleCards.length]?.$id;
    const gradedId = graded.$id;
    const nextBox = good ? Math.min(4, graded.box + 1) : Math.max(1, graded.box - 1);
    const selected = doc.current.selectedBox;
    const remaining = deck.cards.filter((card) => {
      const box = card.$id === gradedId ? nextBox : card.box;
      return selected === 0 || box === selected;
    });
    const nextIndex = nextId !== gradedId ? remaining.findIndex((card) => card.$id === nextId) : -1;
    doc.change((tx) => {
      const handle = tx.at(graded);
      handle.box.set(nextBox);
      handle.lastReviewed.set(new Date().toISOString());
      tx.fields.cardIndex.set(nextIndex >= 0 ? nextIndex : Math.min(previousIndex, Math.max(0, remaining.length - 1)));
    });
    flipped = false;
  }

  function shuffleIds(ids: string[]): string[] {
    const next = [...ids];
    for (let index = next.length - 1; index > 0; index -= 1) {
      const swap = Math.floor(Math.random() * (index + 1));
      const left = next[index]!;
      const right = next[swap]!;
      next[index] = right;
      next[swap] = left;
    }
    return next;
  }

  function shuffleDeck(): void {
    const deck = activeDeck;
    if (!deck) return;
    const order = shuffleIds(deck.cards.map((card) => card.$id));
    const firstId = deck.cards[0]?.$id;
    doc.change((tx) => {
      const cards = tx.at(deck).cards;
      let anchor: string | null = null;
      for (const id of order) {
        if (anchor === null) {
          if (firstId && id !== firstId) cards.move(id, { before: firstId });
        } else cards.move(id, { after: anchor });
        anchor = id;
      }
      tx.fields.cardIndex.set(0);
    });
    flipped = false;
  }

  function startAdd(): void {
    draftFront = "";
    draftBack = "";
    editingExisting = false;
    editing = true;
  }

  function startEdit(): void {
    if (!currentCard) return;
    draftFront = currentCard.front;
    draftBack = currentCard.back;
    editingExisting = true;
    editing = true;
    flipped = false;
  }

  function saveCard(): void {
    const deck = activeDeck;
    if (!deck) return;
    const front = draftFront.trim();
    const back = draftBack.trim();
    if (!front || !back) return;
    if (editingExisting && currentCard) {
      const card = currentCard;
      doc.change((tx) => {
        tx.at(card).front.replace(front);
        tx.at(card).back.replace(back);
      });
    } else {
      doc.change((tx) => {
        tx.at(deck).cards.insert({ front, back, box: 1 });
        tx.fields.selectedBox.set(0);
        tx.fields.cardIndex.set(deck.cards.length);
      });
      flipped = false;
    }
    editing = false;
  }

  function removeCard(): void {
    const deck = activeDeck;
    const card = currentCard;
    if (!deck || !card) return;
    const nextIndex = Math.min(doc.current.cardIndex, Math.max(0, deck.cards.length - 2));
    doc.change((tx) => {
      tx.at(deck).cards.remove(card.$id);
      tx.fields.cardIndex.set(nextIndex);
    });
    flipped = false;
  }

  function addDeck(): void {
    const name = draftDeck.trim();
    if (!name) return;
    const deckKey = crypto.randomUUID();
    doc.change((tx) => {
      tx.fields.decks.insert({ deckKey, name, cards: [] });
      tx.fields.selectedDeckId.set(deckKey);
      tx.fields.selectedBox.set(0);
      tx.fields.cardIndex.set(0);
    });
    flipped = false;
    draftDeck = "";
    addingDeck = false;
  }

  function handleKeyDown(event: KeyboardEvent): void {
    if (event.target instanceof HTMLElement && event.target.closest('input, textarea, select, [role="combobox"], [role="tab"], [role="option"], [contenteditable="true"]')) return;
    if (event.altKey || event.ctrlKey || event.metaKey || event.repeat) return;
    if (dialogOpen) return;
    if (event.code === "Space") {
      if (event.target instanceof HTMLElement && event.target.closest('button, [role="button"]')) return;
      event.preventDefault();
      flipCard();
    } else if (event.key === "1") {
      event.preventDefault();
      grade(false);
    } else if (event.key === "2") {
      event.preventDefault();
      grade(true);
    }
  }

  $effect(() => {
    const cardId = currentCard?.$id ?? null;
    const cardChanged = lastCardId !== null && cardId !== lastCardId;
    lastCardId = cardId;
    const instant = !initialized || prefersReducedMotion.current || cardChanged;
    void rotation.set(rotationTarget, { duration: instant ? 0 : FLIP_MS, delay: 0 });
    initialized = true;
  });

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
    const unregister = capture.onPrepare(async () => {
      await rotation.set(rotationTarget, { duration: 0, delay: 0 });
      await tick();
    });
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      unregister();
    };
  });

  onDestroy(() => {
    void rotation.set(rotation.target, { duration: 0, delay: 0 });
  });
</script>

<Slop>
  <main class="catalog-shell" data-slop-selection="none" aria-label="Flashcard study machine">
    <header class="drawer-head">
      <span class="deck-mark" aria-hidden="true"><Layers size={23} /></span>
      <div class="brass-plate">
        <Select.Root
          type="single"
          value={activeDeck?.deckKey}
          items={deckItems}
          onValueChange={(value) => { if (value) selectDeck(value); }}
        >
          <Select.Trigger class="deck-select" aria-label="Study deck">
            <span>{activeDeck?.name ?? "Select deck"}</span>
            <ChevronDown size={14} strokeWidth={2.4} data-slop-export="hide" />
          </Select.Trigger>
          <Select.Portal>
            <Select.Content class="select-content" data-slop-export="hide" sideOffset={6}>
              <Select.Viewport>
                {#each doc.current.decks as deck (deck.$id)}
                  <Select.Item value={deck.deckKey} label={deck.name}>
                    {#snippet children({ selected })}
                      <span>{deck.name}</span>
                      {#if selected}<Check size={12} />{/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      </div>
      <Button.Root type="button" class="icon-btn" data-slop-export="hide" onclick={() => { addingDeck = true; draftDeck = ""; }} aria-label="Add deck">
        <Plus size={16} />
      </Button.Root>
    </header>

    <Tabs.Root
      value={String(doc.current.selectedBox)}
      onValueChange={(value) => {
        if (value === undefined) return;
        const box = Number(value);
        if (Number.isInteger(box) && box >= 0 && box <= 4) selectBox(box);
      }}
    >
      <Tabs.List class="box-rail" aria-label="Learning stages">
        <Tabs.Trigger value="0" class="box-tab">
          All
          <strong>{activeDeck?.cards.length ?? 0}</strong>
        </Tabs.Trigger>
        {#each BOXES as box}
          <Tabs.Trigger value={String(box.id)} class="box-tab">
            {box.label}
            <strong>{boxCounts[box.id]}</strong>
          </Tabs.Trigger>
        {/each}
      </Tabs.List>
    </Tabs.Root>

    <section class="study-well">
      {#if currentCard}
        <button
          type="button"
          class="card-stage"
          onclick={flipCard}
          aria-pressed={flipped}
          aria-label={flipped ? `Answer: ${currentCard.back}. Show question` : `Question: ${currentCard.front}. Show answer`}
        >
          <div class="card-inner" style:transform={`rotateY(${rotation.current}deg)`}>
            <div class="card-face" aria-hidden={flipped}>
              <span class="card-index">Question <span>{cardIndex + 1} / {visibleCards.length}</span></span>
              <strong>{currentCard.front}</strong>
              <em>Think it through. Tap to reveal.</em>
            </div>
            <div class="card-face card-back" aria-hidden={!flipped}>
              <span class="card-index">Answer <span>{boxLabel}</span></span>
              <strong>{currentCard.back}</strong>
              <em>How did you do?</em>
            </div>
          </div>
        </button>
      {:else}
        <div class="empty-deck">
          <strong>A fresh start</strong>
          <p>{doc.current.selectedBox === 0 ? "Add a card to start this deck." : "Nothing in this box yet."}</p>
          <Button.Root type="button" class="empty-action" data-slop-export="hide" onclick={() => { if (activeDeck) startAdd(); else addingDeck = true; }}>{activeDeck ? "Add a card" : "Create a deck"}</Button.Root>
        </div>
      {/if}
    </section>

    <footer class="study-bar" data-slop-export="hide">
      <div class="review-actions">
        {#if flipped && currentCard}
          <Button.Root type="button" class="grade grade-again" onclick={() => grade(false)}><RotateCw size={16} />Again <kbd>1</kbd></Button.Root>
          <Button.Root type="button" class="grade grade-good" onclick={() => grade(true)}><Check size={16} />Got it <kbd>2</kbd></Button.Root>
        {:else}
          <Button.Root type="button" class="grade reveal" onclick={flipCard} disabled={!currentCard}>Reveal answer <kbd>Space</kbd></Button.Root>
        {/if}
      </div>
      <div class="tools">
        <span>{currentCard ? `${cardIndex + 1} of ${visibleCards.length}` : "No cards"}</span>
        <Button.Root type="button" class="icon-btn" onclick={shuffleDeck} disabled={!activeDeck?.cards.length} aria-label="Shuffle deck"><Shuffle size={16} /></Button.Root>
        <Button.Root type="button" class="icon-btn" onclick={startEdit} disabled={!currentCard} aria-label="Edit card"><Pencil size={16} /></Button.Root>
        <Button.Root type="button" class="icon-btn" onclick={() => { if (activeDeck) startAdd(); else addingDeck = true; }} aria-label="Add card"><Plus size={16} /></Button.Root>
        <Button.Root type="button" class="icon-btn icon-danger" onclick={removeCard} disabled={!currentCard} aria-label="Delete card"><Trash2 size={16} /></Button.Root>
      </div>
    </footer>
  </main>

  <Dialog.Root bind:open={addingDeck}>
    <Dialog.Portal>
      <Dialog.Overlay class="dialog-overlay" data-slop-export="hide" />
      <Dialog.Content class="dialog-card" aria-labelledby="add-deck-title" data-slop-export="hide">
        <div class="dialog-head">
          <Dialog.Title id="add-deck-title">New deck</Dialog.Title>
          <Dialog.Close class="dialog-close" aria-label="Close dialog"><X size={16} /></Dialog.Close>
        </div>
        <form onsubmit={(event) => { event.preventDefault(); addDeck(); }}>
          <label class="form-field">
            <span>DECK NAME</span>
            <input bind:value={draftDeck} placeholder="Typography" required />
          </label>
          <div class="dialog-actions">
            <Button.Root type="button" class="btn-secondary" onclick={() => { addingDeck = false; }}>Cancel</Button.Root>
            <Button.Root type="submit" class="btn-primary">Create deck</Button.Root>
          </div>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  <Dialog.Root bind:open={editing}>
    <Dialog.Portal>
      <Dialog.Overlay class="dialog-overlay" data-slop-export="hide" />
      <Dialog.Content class="dialog-card" aria-labelledby="edit-card-title" data-slop-export="hide">
        <div class="dialog-head">
          <Dialog.Title id="edit-card-title">{editingExisting ? "Edit card" : "New card"}</Dialog.Title>
          <Dialog.Close class="dialog-close" aria-label="Close dialog"><X size={16} /></Dialog.Close>
        </div>
        <form onsubmit={(event) => { event.preventDefault(); saveCard(); }}>
          <label class="form-field">
            <span>FRONT</span>
            <textarea bind:value={draftFront} rows="2" required></textarea>
          </label>
          <label class="form-field">
            <span>BACK</span>
            <textarea bind:value={draftBack} rows="2" required></textarea>
          </label>
          <div class="dialog-actions">
            <Button.Root type="button" class="btn-secondary" onclick={() => { editing = false; }}>Cancel</Button.Root>
            <Button.Root type="submit" class="btn-primary">{editingExisting ? "Save card" : "Add card"}</Button.Root>
          </div>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  {#snippet exportView()}
    <article class="catalog-shell export-shell" aria-label="Exported flashcards">
      <header class="drawer-head">
        <div class="brass-plate">
          <span class="deck-select"><span>{activeDeck?.name ?? "Flashcards"}</span></span>
        </div>
      </header>
      <div class="box-rail" aria-label="Learning stages">
        {#each exportStages as stage}
          <span class="box-tab" data-state={doc.current.selectedBox === stage.id ? "active" : "inactive"}>
            {stage.label}
            <strong>{stage.count}</strong>
          </span>
        {/each}
      </div>
      <section class="study-well">
        {#if currentCard}
          <div class="static-card">
            {#if flipped}
              <span class="card-index">Answer · {boxLabel}</span>
              <strong>{currentCard.back}</strong>
              <span class="card-index"></span>
            {:else}
              <span class="card-index">Question · {cardIndex + 1} / {visibleCards.length}</span>
              <strong>{currentCard.front}</strong>
              <span class="card-index"></span>
            {/if}
          </div>
        {:else}
          <div class="empty-deck">
            <strong>A fresh start</strong>
            <p>Add a card to start this box.</p>
          </div>
        {/if}
      </section>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="icon-surface" aria-hidden="true">
      <svg class="icon-graphic" viewBox="0 0 512 512" fill="none">
        <rect x="30" y="30" width="452" height="452" rx="104" fill="var(--slop-wood)" />
        <rect x="120" y="102" width="300" height="286" rx="28" transform="rotate(10 120 102)" fill="var(--slop-brassHi)" />
        <rect x="76" y="128" width="310" height="282" rx="28" fill="var(--slop-card)" />
        <path d="M173 220C173 180 256 173 260 218C264 250 221 248 221 282" stroke="var(--slop-wood)" stroke-width="22" stroke-linecap="round" />
        <circle cx="221" cy="322" r="12" fill="var(--slop-wood)" />
        <circle cx="375" cy="377" r="70" fill="var(--slop-good)" />
        <path d="M343 377L366 400L407 351" stroke="var(--slop-ink)" stroke-width="16" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </div>
  {/snippet}
</Slop>
