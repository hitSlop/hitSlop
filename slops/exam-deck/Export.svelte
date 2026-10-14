<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { deckView } from "./deck";
  import Header from "./Header.svelte";
  import Readouts from "./Readouts.svelte";

  let { snapshot }: CaptureProps<typeof schema> = $props();

  // A fresh capture page: the readouts as of the moment it renders, without the session UI.
  const now = Date.now();
  const offset = -new Date(now).getTimezoneOffset();
  const view = $derived(deckView(snapshot, now, offset));
</script>

<main class="deck-sheet deck-export">
  <div class="deck-timing" aria-hidden="true"></div>
  <div class="deck-body">
    <Header {view} title={snapshot.title} />
    <Readouts data={snapshot} {view} />
    <p class="deck-export-foot">{snapshot.cards.length} cards · {Math.round(snapshot.scheduler.requestRetention * 100)}% target retention · {snapshot.dailyMinutes} min a day</p>
  </div>
</main>
