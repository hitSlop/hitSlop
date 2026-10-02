<script lang="ts">
  import doc from "./schema";
  import { ui } from "./ui.svelte";
  import { studyView } from "./study";

  const { activeDeck, visibleCards, cardIndex, currentCard, boxLabel, stages } = $derived(studyView(doc.current.decks, ui));
</script>

<article class="catalog-shell export-shell" aria-label="Exported flashcards">
  <header class="drawer-head">
    <div class="brass-plate">
      <span class="deck-select"><span>{activeDeck?.name ?? "Flashcards"}</span></span>
    </div>
  </header>
  <div class="box-rail" aria-label="Learning stages">
    {#each stages as stage}
      <span class="box-tab" data-state={ui.box === stage.id ? "active" : "inactive"}>
        {stage.label}
        <strong>{stage.count}</strong>
      </span>
    {/each}
  </div>
  <section class="study-well">
    {#if currentCard}
      <div class="static-card">
        {#if ui.flipped}
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
