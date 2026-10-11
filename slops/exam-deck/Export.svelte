<script lang="ts">
  import doc from "./schema";
  import { deckView } from "./deck";
  import Header from "./Header.svelte";
  import Readouts from "./Readouts.svelte";

  // A fresh capture page: the readouts as of the moment it renders, without the session UI.
  const now = Date.now();
  const offset = -new Date(now).getTimezoneOffset();
  const view = $derived(deckView(doc.current, now, offset));
</script>

<main class="deck-sheet deck-export">
  <div class="deck-timing" aria-hidden="true"></div>
  <div class="deck-body">
    <Header {view} />
    <Readouts data={doc.current} {view} />
    <p class="deck-export-foot">{doc.current.cards.length} cards · {Math.round(doc.current.scheduler.requestRetention * 100)}% target retention · {doc.current.dailyMinutes} min a day</p>
  </div>
</main>
