<script lang="ts">
  import type { Handle, StringNode } from "hitslop";
  import { formatDay, type DeckView } from "./deck";

  // The editor passes the title's field handle and `onexam`; a static view passes only `title`.
  interface Props { view: DeckView; title: string; field?: Handle<StringNode>; onexam?: () => void }
  const { view, title, field, onexam }: Props = $props();
  const exam = $derived(view.forecast?.exam);
  const when = $derived.by(() => {
    if (!exam) return "";
    if (exam.daysLeft < 0) return "passed";
    if (exam.daysLeft === 0) return "today";
    return exam.daysLeft === 1 ? "tomorrow" : `in ${exam.daysLeft} days`;
  });
</script>

<header class="deck-head">
  <div class="deck-form-line">
    <span class="deck-form-name">Exam Deck</span>
    <span class="deck-form-code">Answer sheet · FSRS-6</span>
  </div>
  <div class="deck-fields">
    <label class="deck-field deck-field-subject">
      <span class="deck-field-label">Subject</span>
      {#if field}
        <input class="deck-pencil-input" aria-label="Subject" maxlength="80" bind:value={field.value} />
      {:else}
        <span class="deck-pencil">{title}</span>
      {/if}
    </label>
    <div class="deck-field deck-field-exam">
      <span class="deck-field-label">Exam date</span>
      {#if field}
        <button class="deck-exam-button" onclick={onexam} aria-label="Exam date: {exam ? `${exam.assumed ? "not set" : formatDay(exam.date, "long")}, ${when}` : "not set"}. Change in settings.">
          {#if exam && !exam.assumed}
            <span class="deck-pencil">{formatDay(exam.date, "long")}</span><small>{when}</small>
          {:else}
            <span class="deck-unset">not set</span><small>choose</small>
          {/if}
        </button>
      {:else if exam && !exam.assumed}
        <span class="deck-exam-static"><span class="deck-pencil">{formatDay(exam.date, "long")}</span><small>{when}</small></span>
      {:else}
        <span class="deck-exam-static"><span class="deck-unset">not set</span></span>
      {/if}
    </div>
  </div>
</header>
