<script lang="ts">
  import doc from "./schema";
  import { clampWeight } from "./balance";
  import Scale from "./Scale.svelte";
  import { beamTilt, totalWeight } from "./balance";
  import { STATUSES } from "./statuses";

  const pros = $derived(doc.current.factors.filter((item) => item.side === "pro"));

  const cons = $derived(doc.current.factors.filter((item) => item.side === "con"));

  const proTotal = $derived(totalWeight(pros));

  const conTotal = $derived(totalWeight(cons));

  const tiltTarget = $derived(beamTilt(proTotal, conTotal));

  const statusLabel = $derived(STATUSES.find(item => item.value === doc.current.status)?.label ?? "Evaluating");
</script>

<article class="exportLetter" aria-label="Exported decision balance">
  <header class="letterhead">
    <div class="metaRow">
      <span class="stamp">Decision Balance</span>
      <span class="dateField">{doc.current.date}</span>
    </div>
    <div class="questionRow">
      <span class="whether">Whether to</span>
      <h1 class="question">{doc.current.question.trim() || "state the choice clearly…"}</h1>
    </div>
    <Scale tilt={tiltTarget} {proTotal} {conTotal} />
  </header>

  <div class="spread">
    <section class="column" data-side="for" aria-labelledby="export-for">
      <div class="columnHead">
        <div>
          <h2 id="export-for" class="columnTitle">Pros</h2>
          <span class="columnHint">Reasons in favor</span>
        </div>
        <span class="columnTotal">{proTotal}</span>
      </div>
      <ul class="list">
        {#each pros as item (item.$id)}
          <li class="row">
            <span class="reason">{item.text.trim() || "Untitled reason"}</span>
            <span class="weightNum">{clampWeight(item.weight)}</span>
          </li>
        {:else}
          <li class="empty">No motives recorded.</li>
        {/each}
      </ul>
    </section>
    <section class="column" data-side="against" aria-labelledby="export-against">
      <div class="columnHead">
        <div>
          <h2 id="export-against" class="columnTitle">Cons</h2>
          <span class="columnHint">Reasons against</span>
        </div>
        <span class="columnTotal">{conTotal}</span>
      </div>
      <ul class="list">
        {#each cons as item (item.$id)}
          <li class="row">
            <span class="reason">{item.text.trim() || "Untitled reason"}</span>
            <span class="weightNum">{clampWeight(item.weight)}</span>
          </li>
        {:else}
          <li class="empty">No objections recorded.</li>
        {/each}
      </ul>
    </section>
  </div>

  <footer class="foot">
    <div class="seal" aria-label="Verdict status {statusLabel}">
      <span class="sealFace">{statusLabel}</span>
    </div>
    <div class="verdict">
      <span class="verdictLabel">Verdict</span>
      {#if doc.current.verdict.trim()}
        <p class="verdictExport">{doc.current.verdict}</p>
      {:else}
        <p class="empty">No conclusion recorded.</p>
      {/if}
    </div>
  </footer>
</article>
