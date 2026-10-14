<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import type schema from "./schema";
  import SheetHead from "./SheetHead.svelte";
  import Ladder from "./Ladder.svelte";
  import Record from "./Record.svelte";
  import { localDay, practiceView } from "./practice";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const today = localDay(new Date());
  const view = $derived(practiceView(snapshot, today));
</script>

<main class="ps-desk ps-export">
  <article class="ps-sheet">
    <header class="ps-head">
      <SheetHead header={view.header} />
      <p class="ps-stamp" aria-label={`Practising at ${view.tempo} beats per minute`}>
        <span class="ps-stamp-note" aria-hidden="true"><svg width="14" height="19" viewBox="0 0 14 19"><ellipse cx="5" cy="15" rx="4.5" ry="3" fill="currentColor" transform="rotate(-20 5 15)"/><path d="M9 14V1" fill="none" stroke="currentColor" stroke-width="2"/></svg> =</span>
        <strong>{view.tempo}</strong>
        <small>{view.atGoal ? "goal reached" : `goal ${view.ladder.goal}`}</small>
      </p>
    </header>
    <div class="ps-score">
      <p class="ps-label">ABC notation</p>
      <pre class="ps-export-notation">{previewText(snapshot.abc, preview, 500)}</pre>
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

<style>.ps-export-notation { white-space: pre-wrap; overflow-wrap: anywhere; font-family: monospace; font-size: 15px; line-height: 1.8; padding: 24px 0; margin: 0; color: var(--slop-ink); }</style>
