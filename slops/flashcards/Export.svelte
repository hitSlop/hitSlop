<script lang="ts">
  import doc from "./schema";
  import { studyView } from "./study";

  // The export is a fresh page with no session: it shows the first deck, every card, and the question side.
  const { activeDeck, visibleCards, cardIndex, currentCard, stages } = $derived(
    studyView(doc.current, { deckKey: "", box: 0, cardIndex: 0 }),
  );
</script>

<article class="catalog-shell export-shell" aria-label="Exported flashcards">
  <header class="drawer-head">
    <div class="brass-plate">
      <span class="deck-select"><span>{activeDeck?.name ?? "Flashcards"}</span></span>
    </div>
  </header>
  <div class="box-rail" aria-label="Learning stages">
    {#each stages as stage}
      <span class="box-tab" data-state={stage.id === 0 ? "active" : "inactive"}>
        {stage.label}
        <strong>{stage.count}</strong>
      </span>
    {/each}
  </div>
  <section class="study-well">
    {#if currentCard}
      <div class="static-card">
        <span class="card-index">Question · {cardIndex + 1} / {visibleCards.length}</span>
        <strong>{currentCard.front}</strong>
        <span class="card-index"></span>
      </div>
    {:else}
      <div class="empty-deck">
        <strong>A fresh start</strong>
        <p>Add a card to start this deck.</p>
      </div>
    {/if}
  </section>
</article>
