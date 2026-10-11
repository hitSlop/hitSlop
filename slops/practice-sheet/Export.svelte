<script lang="ts">
  import doc from "./schema";
  import Engraved from "./Engraved.svelte";
  import SheetHead from "./SheetHead.svelte";
  import Ladder from "./Ladder.svelte";
  import Record from "./Record.svelte";
  import { localDay, practiceView } from "./practice";

  const today = localDay(new Date());
  const view = $derived(practiceView(doc.current, today));
</script>

<main class="ps-desk ps-export">
  <article class="ps-sheet">
    <header class="ps-head">
      <SheetHead header={view.header} />
      <p class="ps-stamp" aria-label={`Practising at ${view.tempo} beats per minute`}>
        <span class="ps-stamp-note" aria-hidden="true">♩ =</span>
        <strong>{view.tempo}</strong>
        <small>{view.atGoal ? "goal reached" : `goal ${view.ladder.goal}`}</small>
      </p>
    </header>
    <div class="ps-score">
      <Engraved abc={doc.current.abc} width={720} />
    </div>
    <footer class="ps-export-foot">
      <section class="ps-export-ladder" aria-label="Tempo ladder">
        <Ladder rungs={view.rungs} tempo={view.tempo} />
        <div>
          <p class="ps-label">Tempo ladder</p>
          <p class="ps-export-line">{view.ladder.start} → {view.ladder.goal} bpm, +{view.ladder.step} per clean run</p>
          <p class="ps-export-line">{view.atGoal ? "At the goal tempo." : `${view.ladder.goal - view.tempo} bpm to go.`}</p>
        </div>
      </section>
      <section class="ps-record" aria-label="Practice record">
        <p class="ps-label">Practice record</p>
        <Record {view} />
      </section>
    </footer>
  </article>
</main>
