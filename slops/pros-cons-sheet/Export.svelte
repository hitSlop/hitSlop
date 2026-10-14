<script lang="ts">
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { balanceView, clampWeight } from "./balance";
  import Scale from "./Scale.svelte";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const { pros, cons, proTotal, conTotal, tilt, statusLabel } = $derived(balanceView(snapshot));
</script>

<article class="exportLetter" aria-label="Exported decision balance">
  <header class="letterhead">
    <div class="metaRow">
      <span class="stamp">Decision Balance</span>
      <span class="dateField">{snapshot.date}</span>
    </div>
    <div class="questionRow">
      <span class="whether">Whether to</span>
      <h1 class="question">{snapshot.question.trim() || "state the choice clearly…"}</h1>
    </div>
    <Scale {tilt} {proTotal} {conTotal} />
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
        {#each (preview ? (pros).slice(0, 4) : pros) as item (item.$id)}
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
        {#each (preview ? (cons).slice(0, 4) : cons) as item (item.$id)}
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
      {#if snapshot.verdict.trim()}
        <p class="verdictExport">{snapshot.verdict}</p>
      {:else}
        <p class="empty">No conclusion recorded.</p>
      {/if}
    </div>
  </footer>
</article>
